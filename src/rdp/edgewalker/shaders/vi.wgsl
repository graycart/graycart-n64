// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// VI scan-out: RGBA16 framebuffer bytes to packed RGBA8888.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Video Interface
//   https://n64brew.dev/wiki/Video_Interface
//   5-bit channels expand by (v << 3) | (v >> 2).
//   The RGBA5551 alpha bit expands to 0x00 or 0xFF.
//   This is the unfiltered scan-out. It does not fold the two RDRAM 9th bits
//   into a 5/5/5/3 analog pixel. Gamma, gamma dither, divot, dedither, and
//   any AA_MODE other than REPLICATE are host traps and are not applied here.
//
// Supported this wave (writes rgba8888):
//   color_type 2 (RGBA16). One thread per output pixel.
// Unsupported (no writes; the host traps): color_type other than 2,
//   including VI off and RGBA32. Width or height 0 writes nothing (empty frame).
// The shader cannot trap. A pixel whose halfword sits past `color` expands
// as 0, matching a short RDRAM read, and the output pixel is still stored.
//
// Source addressing (N64brew Video Interface hardware spec):
//   stride_bytes = width * 2
//   byte_addr = origin + y * stride_bytes + x * 2
//   origin is a byte offset into `color` (VI_ORIGIN when the SSBO mirrors
//   RDRAM). The halfword is big-endian in that byte stream.
//   `color` u32 element N holds RDRAM bytes 4N..4N+3 with byte 4N in bits [7:0].
//
// Output word at rgba8888[y * width + x], little-endian RGBA bytes:
//   bits [7:0] = R, [15:8] = G, [23:16] = B, [31:24] = A
//   so a memcpy into `Vec<u8>` is R, G, B, A, matching `Rgba8888Frame.rgba`.
//   Example: halfword 0xF801 (r = 31, a = 1) → bytes FF 00 00 FF.
//
// Entry point: vi_main
// Workgroup: 64. Dispatch ceil(width * height / 64) workgroups.
//
// @group(0) binding table (all storage):
//   binding 0  color     read        array<u32>       RDRAM byte mirror
//   binding 1  coverage  read        array<u32>       not a pixel source
//                        index = byte address, low 8 bits = hidden bit 0 or 1
//   binding 2  rgba8888  read_write  array<u32>       one LE RGBA8888 word
//                                                    per output pixel
//   binding 3  vi_params read        array<ViParams>  stride 16
//                                    only element 0 is read
//
// ViParams element 0:
//   0 origin:u32
//   4 width:u32       pixels (VI_WIDTH)
//   8 height:u32      source rows (scanout_height)
//   12 color_type:u32 2 = RGBA16
//
// Layout vs the previous stub:
//   Bindings 0, 1, and 2 are the same variables.
//   Binding 3 `vi_params` is new. Without it the pass has no width or origin.
//   `coverage` stays bound so the 9th-bit buffer remains in the group. REPLICATE
//   does not read it into the pixel.

struct ViParams {
  origin: u32,
  width: u32,
  height: u32,
  color_type: u32,
}

@group(0) @binding(0) var<storage, read> color: array<u32>;
@group(0) @binding(1) var<storage, read> coverage: array<u32>;
@group(0) @binding(2) var<storage, read_write> rgba8888: array<u32>;
@group(0) @binding(3) var<storage, read> vi_params: array<ViParams>;

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

fn load_be_u16(byte_addr: u32) -> u32 {
  if (byte_addr & 1u) != 0u || byte_addr >= 0xFFFFFFFEu {
    return 0u;
  }
  let nbytes = arrayLength(&color) * 4u;
  if byte_addr + 1u >= nbytes {
    return 0u;
  }
  let bits = color[byte_addr >> 2u];
  let shift = (byte_addr & 2u) * 8u;
  let packed = (bits >> shift) & 0xFFFFu;
  let hi = packed & 0xFFu;
  let lo = (packed >> 8u) & 0xFFu;
  return (hi << 8u) | lo;
}

fn replicate5(v: u32) -> u32 {
  let c = v & 0x1Fu;
  return (c << 3u) | (c >> 2u);
}

fn expand_5551(pix: u32) -> u32 {
  let r = replicate5(pix >> 11u);
  let g = replicate5(pix >> 6u);
  let b = replicate5(pix >> 1u);
  var a = 0u;
  if (pix & 1u) == 1u {
    a = 0xFFu;
  }
  return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(64)
fn vi_main(@builtin(global_invocation_id) id: vec3<u32>) {
  // Binding 1 stays live. REPLICATE does not fold hidden bits into the pixel.
  if arrayLength(&coverage) == 0xFFFFFFFFu || arrayLength(&vi_params) == 0u {
    return;
  }
  let p = vi_params[0];
  if p.color_type != 2u || p.width == 0u || p.height == 0u {
    return;
  }
  if u32_mul_overflows(p.width, p.height) {
    return;
  }
  let index = id.x;
  let pixel_count = p.width * p.height;
  if index >= pixel_count || index >= arrayLength(&rgba8888) {
    return;
  }
  let y = u32(floor(f32(index) / f32(p.width)));
  let x = index - y * p.width;
  let stride = p.width * 2u;
  if y != 0u && u32_mul_overflows(y, stride) {
    rgba8888[index] = expand_5551(0u);
    return;
  }
  let row_off = y * stride;
  if p.origin > 0xFFFFFFFFu - row_off {
    rgba8888[index] = expand_5551(0u);
    return;
  }
  let row = p.origin + row_off;
  if x > 0x7FFFFFFFu || row > 0xFFFFFFFFu - x * 2u {
    rgba8888[index] = expand_5551(0u);
    return;
  }
  let pix = load_be_u16(row + x * 2u);
  rgba8888[index] = expand_5551(pix);
}
