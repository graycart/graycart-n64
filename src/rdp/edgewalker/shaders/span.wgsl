// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// Span stage. This wave rasterizes one FILL-cycle Fill Rectangle into an
// RGBA16 color image. Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor
//   https://n64brew.dev/wiki/Reality_Display_Processor
// Cited: N64brew — Reality Display Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   Fill Rectangle 0x36, Set Fill Color 0x37, Set Scissor 0x2D,
//   Set Other Modes 0x2F (cycle type bits 53:52), Set Color Image 0x3F.
//
// Supported this wave (writes color):
//   cycle type FILL (3) and color-image size 2 (RGBA16 / 16-bit).
//   One rectangle: `params[0]`. The host submits later rectangles as later
//   dispatches so painter's order matches the serial RDP.
// Unsupported (no pixel writes; the host Rust traps before dispatch):
//   cycle type 1-cycle (0), 2-cycle (1), and COPY (2);
//   color-image sizes other than 16-bit (4-bit traps under N64 RDP FILL);
//   triangles, texture rectangles, TMEM, the combiner, the blender,
//   anti-alias coverage, Z compare / Z update, and RGB dither.
//   COPY rectangles and non-FILL cycles must not be sent here.
// The shader cannot trap. A thread outside the rectangle, outside the
// scissor, or aimed at a byte the color buffer does not hold returns.
// A supported in-range pixel is stored.
//
// FILL pixel range (`ulx >> 2 ..= ceil(lrx)` inclusive):
//   x0 = (rect_ulx & 0xFFF) >> 2
//   y0 = (rect_uly & 0xFFF) >> 2
//   x1 = ceil_10_2(rect_lrx & 0xFFF)     // inclusive
//   y1 = ceil_10_2(rect_lry & 0xFFF)     // inclusive
//   ceil_10_2(v) = v>>2, plus 1 when the low 2 bits are not 0.
//   If x0 > x1 or y0 > y1 the rectangle is empty and nothing is stored.
//   Visited pixels are x = x0 ..= x1, y = y0 ..= y1, then scissor-tested.
//   12-bit fields are unsigned per N64 RDP Fill Rectangle encoding.
//
// FILL scissor:
//   subpixels ignored (>> 2); right edge inclusive; bottom edge exclusive.
//   x >= (scissor_ulx >> 2) && x <= (scissor_lrx >> 2)
//   y >= (scissor_uly >> 2) && y <  (scissor_lry >> 2)
//   x < 0 or y < 0 is rejected.
//   scissor_field == 0 keeps every line. Otherwise the line is kept when
//   ((y & 1) == 1) equals (scissor_odd != 0). Fields are the latched
//   `RdpScissor` values (u16 stored in the low bits), not a raw command word.
//
// RGBA16 FILL store (color image size 2):
//   width_px = Set Color Image width field + 1   (host passes pixels)
//   pix = y * width_px + x
//   byte_addr = color_dram_addr + pix * 2
//   even x stores fill_color[31:16]; odd x stores fill_color[15:0].
//   Fill color 0x11112222 therefore alternates 0x1111, 0x2222 across x.
//   The color SSBO is a byte mirror of RDRAM. Each u32 is the little-endian
//   view of four RDRAM bytes (byte 0 in bits [7:0]). A halfword is written
//   big-endian inside that stream: 0x1111 at addr 0 becomes bytes 11 11,
//   and with the following 0x2222 the first u32 is 0x22221111.
//   Neighboring halfwords in the same u32 are preserved (CAS loop on
//   that word's 16 bits only).
//   FILL does not update the 9th-bit coverage side array, leaving hidden bits
//   untouched on RGBA16 FILL. This shader does not write `coverage` or `depth`.
//
// M6 smoke (fill_rectangle_smoke_cmds): rect lrx = 4, other rect fields 0;
// scissor lrx = 4, lry = 4; width_px = 16; dram = 0; fill = 0x11112222.
//   ceil_10_2(4) = 1, ceil_10_2(0) = 0 → pixels (0,0) and (1,0)
//   scissor keeps x <= 1 and y < 1
//   addr 0 = 0x1111, addr 2 = 0x2222, addr 4 unchanged.
//
// Entry point: span_main
// Workgroup: 64. One thread per pixel of the inclusive rectangle.
//   rect_w = x1 - x0 + 1, rect_h = y1 - y0 + 1
//   dispatch ceil(rect_w * rect_h / 64) workgroups
//   id.x = ly * rect_w + lx
//   ly = floor(f32(id.x) / f32(rect_w)), lx = id.x - ly * rect_w
//   x = x0 + lx, y = y0 + ly
// Integer division in this stage faults NVIDIA's Vulkan compiler (NVVM)
// while the compute pipeline is created. The 12-bit rectangle domain fits
// in an f32 mantissa, so that floor matches the unsigned row. Overflow
// guards compare a multiply instead of dividing.
//
// @group(0) binding table (all storage):
//   binding 0  params    read        array<SpanParams>  stride 80
//                        only element 0 is the rectangle
//   binding 1  tmem      read        array<u32>         unused on FILL
//                        4 KiB TMEM is 1024 words when the host uploads it
//   binding 2  color     read_write  array<atomic<u32>> byte mirror of RDRAM
//   binding 3  coverage  read_write  array<u32>         not written on FILL
//                        index = RDRAM byte address
//                        low 8 bits = hidden bit 0 or 1 (one element per byte)
//   binding 4  depth     read_write  array<u32>         not written on FILL
//
// SpanParams element 0, byte offsets:
//    0 other_modes_lo:u32     bits [31:0] of latched other_modes
//    4 other_modes_hi:u32     bits [63:32]; cycle type is bits [21:20]
//                             (other_modes bits [53:52])
//    8 combine_lo:u32         latched combine, low half (unused on FILL)
//   12 combine_hi:u32
//   16 fill_color:u32
//   20 color_dram_addr:u32    byte offset into `color` (RDRAM address when
//                             the SSBO mirrors RDRAM from 0). Must be even.
//   24 color_width_px:u32     pixels per row, already (width field + 1)
//   28 color_size:u32         2 = 16-bit. Any other size writes nothing.
//   32 rect_ulx:u32           s10.2, masked to 12 bits in the shader
//   36 rect_uly:u32
//   40 rect_lrx:u32
//   44 rect_lry:u32
//   48 scissor_ulx:u32
//   52 scissor_uly:u32
//   56 scissor_lrx:u32
//   60 scissor_lry:u32
//   64 scissor_field:u32      0 = all lines, nonzero = interlace
//   68 scissor_odd:u32        nonzero = keep odd y when field is set
//   72 _pad0:u32
//   76 _pad1:u32
//
// Layout vs the previous stub:
//   Binding numbers 0..4 are unchanged.
//   SpanParams grew from 16 bytes (other_modes + combine only) to 80 bytes.
//   `color` is now array<atomic<u32>> so two pixels can update the two
//   halfwords of one word. Element size is still 4 bytes; the host buffer
//   is a plain RDRAM byte mirror (little-endian u32 words).
//   tmem, coverage, and depth stay bound and are not pixel sources on FILL.

struct SpanParams {
  other_modes_lo: u32,
  other_modes_hi: u32,
  combine_lo: u32,
  combine_hi: u32,
  fill_color: u32,
  color_dram_addr: u32,
  color_width_px: u32,
  color_size: u32,
  rect_ulx: u32,
  rect_uly: u32,
  rect_lrx: u32,
  rect_lry: u32,
  scissor_ulx: u32,
  scissor_uly: u32,
  scissor_lrx: u32,
  scissor_lry: u32,
  scissor_field: u32,
  scissor_odd: u32,
  _pad0: u32,
  _pad1: u32,
}

@group(0) @binding(0) var<storage, read> params: array<SpanParams>;
@group(0) @binding(1) var<storage, read> tmem: array<u32>;
@group(0) @binding(2) var<storage, read_write> color: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> coverage: array<u32>;
@group(0) @binding(4) var<storage, read_write> depth: array<u32>;

fn ceil_10_2(v: i32) -> i32 {
  let i = v >> 2;
  if (v & 3) == 0 {
    return i;
  }
  return i + 1;
}

fn allow_line(y: i32, field: u32, odd: u32) -> bool {
  if field == 0u {
    return true;
  }
  let odd_line = (y & 1) == 1;
  let want_odd = odd != 0u;
  return odd_line == want_odd;
}

fn fill_pixel_kept(x: i32, y: i32, s: SpanParams) -> bool {
  if x < 0 || y < 0 {
    return false;
  }
  if !allow_line(y, s.scissor_field, s.scissor_odd) {
    return false;
  }
  let ulx = i32(s.scissor_ulx & 0xFFFFu);
  let uly = i32(s.scissor_uly & 0xFFFFu);
  let lrx = i32(s.scissor_lrx & 0xFFFFu);
  let lry = i32(s.scissor_lry & 0xFFFFu);
  return x >= (ulx >> 2) && x <= (lrx >> 2) && y >= (uly >> 2) && y < (lry >> 2);
}

fn fill_half(fill_color: u32, x: i32) -> u32 {
  if (u32(x) & 1u) == 0u {
    return fill_color >> 16u;
  }
  return fill_color & 0xFFFFu;
}

// True when `a * b` does not fit in u32. No integer quotient: NVIDIA's
// compiler faults the device if this stage's SPIR-V contains one.
fn u32_mul_overflows(a: u32, b: u32) -> bool {
  let a_hi = a >> 16u;
  let a_lo = a & 0xFFFFu;
  let b_hi = b >> 16u;
  let b_lo = b & 0xFFFFu;
  if a_hi != 0u && b_hi != 0u {
    return true;
  }
  let mid = a_hi * b_lo + a_lo * b_hi;
  let low = a_lo * b_lo;
  let sum_mid = (low >> 16u) + (mid & 0xFFFFu);
  return (mid >> 16u) + (sum_mid >> 16u) != 0u;
}

fn store_be_u16(byte_addr: u32, half: u32) {
  if (byte_addr & 1u) != 0u || byte_addr >= 0xFFFFFFFEu {
    return;
  }
  let nbytes = arrayLength(&color) * 4u;
  if byte_addr + 1u >= nbytes {
    return;
  }
  let word = byte_addr >> 2u;
  let shift = (byte_addr & 2u) * 8u;
  let hi = (half >> 8u) & 0xFFu;
  let lo = half & 0xFFu;
  let packed = (hi | (lo << 8u)) << shift;
  let keep = ~(0xFFFFu << shift);

  var cur = atomicLoad(&color[word]);
  loop {
    let next = (cur & keep) | packed;
    let res = atomicCompareExchangeWeak(&color[word], cur, next);
    if res.exchanged {
      break;
    }
    cur = res.old_value;
  }
}

@compute @workgroup_size(64)
fn span_main(@builtin(global_invocation_id) id: vec3<u32>) {
  // Keep bindings 1, 3, and 4 in the pipeline layout. FILL does not use them.
  let reserved = arrayLength(&tmem) == 0xFFFFFFFFu
    || arrayLength(&coverage) == 0xFFFFFFFFu
    || arrayLength(&depth) == 0xFFFFFFFFu;
  if reserved || arrayLength(&params) == 0u {
    return;
  }

  let s = params[0];
  let cycle = (s.other_modes_hi >> 20u) & 3u;
  if cycle != 3u || s.color_size != 2u || s.color_width_px == 0u {
    return;
  }
  if (s.color_dram_addr & 1u) != 0u {
    return;
  }

  let ulx = i32(s.rect_ulx & 0xFFFu);
  let uly = i32(s.rect_uly & 0xFFFu);
  let lrx = i32(s.rect_lrx & 0xFFFu);
  let lry = i32(s.rect_lry & 0xFFFu);
  let x0 = ulx >> 2;
  let y0 = uly >> 2;
  let x1 = ceil_10_2(lrx);
  let y1 = ceil_10_2(lry);
  if x0 > x1 || y0 > y1 {
    return;
  }

  let rect_w = u32(x1 - x0 + 1);
  let rect_h = u32(y1 - y0 + 1);
  if rect_w == 0u || u32_mul_overflows(rect_w, rect_h) {
    return;
  }
  let index = id.x;
  if index >= rect_w * rect_h {
    return;
  }
  let span_row = u32(floor(f32(index) / f32(rect_w)));
  let span_col = index - span_row * rect_w;
  let x = x0 + i32(span_col);
  let y = y0 + i32(span_row);
  if !fill_pixel_kept(x, y, s) {
    return;
  }

  let x_px = u32(x);
  let y_px = u32(y);
  if u32_mul_overflows(y_px, s.color_width_px) {
    return;
  }
  let row = y_px * s.color_width_px;
  if row > 0xFFFFFFFFu - x_px {
    return;
  }
  let pix = row + x_px;
  if pix > 0x7FFFFFFFu || s.color_dram_addr > 0xFFFFFFFFu - pix * 2u {
    return;
  }
  let byte_addr = s.color_dram_addr + pix * 2u;
  store_be_u16(byte_addr, fill_half(s.fill_color, x));
}
