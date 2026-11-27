//! Bit-exact frame-hash harness: EdgeWalker (Vulkan compute LLE) matches CpuRdpLle.
//!
//! One command list and the same fresh RDRAM image are submitted to
//! [`super::EdgeWalker`] and [`super::super::CpuRdpLle`]. The VI RGBA8888
//! scanout hash and a raw framebuffer FNV (color bytes plus the 9th-bit side
//! array in that window) must be identical. A one-sided trap, a trap on a
//! seed list, or a hash mismatch fails with both results.
//!
//! Cited: N64brew — Reality Display Processor / Video Interface
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Video_Interface
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6 gate)

use std::fmt;

use crate::bus::Rdram;
use crate::trap::Trap;
use crate::vi::{Vi, ViColorType};

use super::super::{CpuRdpLle, RdpBackend, RdpSyncKind};
use super::EdgeWalker;

/// `Sync Full` (`0x29`). Top two bits of the command byte are ignored by the pipe.
pub const SYNC_FULL: u64 = 0x29 << 56;

/// Hashes that matched for one command list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityReceipt {
    /// [`Vi::scanout_frame_hash`] of the shared VI programming.
    pub scanout_hash: u64,
    /// FNV-1a 64 of the scanout window's color bytes and 9th bits.
    pub raw_fb_fnv: u64,
}

/// Both frame hashes, plus the first byte or 9th-bit address that differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityMismatch {
    pub case_name: &'static str,
    pub cpu_scanout: u64,
    pub edge_scanout: u64,
    pub cpu_raw_fb: u64,
    pub edge_raw_fb: u64,
    pub first_byte_diff: Option<u32>,
    pub first_hidden_diff: Option<u32>,
}

/// EdgeWalker (Vulkan compute LLE) did not match CpuRdpLle.
#[derive(Debug)]
pub enum ParityFailure {
    /// A backend returned [`Trap`] while running `phase`.
    Backend {
        case_name: &'static str,
        phase: &'static str,
        cpu: Result<(), Trap>,
        edge: Result<(), Trap>,
    },
    /// VI scanout itself trapped (filters, reserved TYPE, or a bad origin/stride).
    Scanout {
        case_name: &'static str,
        cpu: Result<u64, Trap>,
        edge: Result<u64, Trap>,
    },
    Mismatch(ParityMismatch),
}

impl fmt::Display for ParityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend {
                case_name,
                phase,
                cpu,
                edge,
            } => write!(
                f,
                "EdgeWalker (Vulkan compute LLE) does not match CpuRdpLle on '{case_name}' during {phase}: CpuRdpLle {}; EdgeWalker {}",
                fmt_side(cpu),
                fmt_side(edge)
            ),
            Self::Scanout {
                case_name,
                cpu,
                edge,
            } => write!(
                f,
                "EdgeWalker (Vulkan compute LLE) scanout does not match CpuRdpLle on '{case_name}': CpuRdpLle {}; EdgeWalker {}",
                fmt_scanout(cpu),
                fmt_scanout(edge)
            ),
            Self::Mismatch(mismatch) => write!(f, "{mismatch}"),
        }
    }
}

impl fmt::Display for ParityMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EdgeWalker (Vulkan compute LLE) does not match CpuRdpLle on '{}': VI scanout hash cpu={:#018x} edge={:#018x}; raw FB FNV cpu={:#018x} edge={:#018x}",
            self.case_name,
            self.cpu_scanout,
            self.edge_scanout,
            self.cpu_raw_fb,
            self.edge_raw_fb
        )?;
        if let Some(addr) = self.first_byte_diff {
            write!(f, "; first byte diff PA {addr:#010x}")?;
        }
        if let Some(addr) = self.first_hidden_diff {
            write!(f, "; first 9th-bit diff PA {addr:#010x}")?;
        }
        Ok(())
    }
}

fn fmt_side(result: &Result<(), Trap>) -> String {
    match result {
        Ok(()) => "ok".to_string(),
        Err(trap) => format!("trapped: {trap}"),
    }
}

fn fmt_scanout(result: &Result<u64, Trap>) -> String {
    match result {
        Ok(hash) => format!("scanout {hash:#018x}"),
        Err(trap) => format!("trapped: {trap}"),
    }
}

/// RGBA16, `AA_MODE` = REPLICATE, origin 0, one output line, 1:1 Y scale.
///
/// `width` is in pixels. A 16-bit stride must be a multiple of 8 bytes
/// (width multiple of 4) or [`Vi::scanout_rgba8888`] traps.
#[must_use]
pub fn rgba16_replicate_scanout(width: u32) -> Vi {
    let mut vi = Vi::new();
    vi.write_reg(0x00, (0b11 << 8) | 2)
        .expect("VI_CTRL RGBA16 REPLICATE");
    vi.write_reg(0x04, 0).expect("VI_ORIGIN");
    vi.write_reg(0x08, width).expect("VI_WIDTH");
    // V_START = 0, V_END = 2 → one active output line.
    vi.write_reg(0x28, 2).expect("VI_V_VIDEO");
    vi.write_reg(0x34, 0x400).expect("VI_Y_SCALE");
    vi
}

/// FillRectangle smoke list from the M5 `submit_fill_rectangle_writes_known_pixel` vector.
///
/// RGBA16 color image, 16 pixels wide, at DRAM 0. FILL cycle, RGB dither off.
/// Fill color `0x1111_2222`. Scissor and rectangle cover x through 1.0 and one row.
#[must_use]
pub fn fill_rectangle_smoke_cmds() -> [u64; 5] {
    let color = (0x3F_u64 << 56) | (2 << 51) | (15 << 32);
    let modes = (0x2F_u64 << 56) | (3 << 52) | (3 << 38);
    let fill = (0x37_u64 << 56) | 0x1111_2222;
    let scissor = (0x2D_u64 << 56) | (4 << 12) | 4;
    let rect = (0x36_u64 << 56) | (4 << 44);
    [color, modes, fill, scissor, rect]
}

/// Run `cmds`, then each sync barrier, on both backends and require bit-exact frames.
///
/// Success is both backends returning `Ok` and equal VI scanout and raw FB hashes.
/// Any trap fails, including when both backends trap the same way: a seed list
/// that stops drawing is not a match.
pub fn compare_edgewalker_to_cpu_rdp_lle(
    case_name: &'static str,
    cmds: &[u64],
    syncs: &[RdpSyncKind],
    vi: &Vi,
) -> Result<ParityReceipt, ParityFailure> {
    let mut cpu = CpuRdpLle::new();
    let mut edge = EdgeWalker::new();
    let mut cpu_ram = Rdram::new(false);
    let mut edge_ram = Rdram::new(false);

    let cpu_submit = cpu.submit(cmds, &mut cpu_ram);
    let edge_submit = edge.submit(cmds, &mut edge_ram);
    if cpu_submit.is_err() || edge_submit.is_err() {
        return Err(ParityFailure::Backend {
            case_name,
            phase: "submit",
            cpu: cpu_submit,
            edge: edge_submit,
        });
    }

    for kind in syncs {
        let phase = sync_phase(*kind);
        let cpu_sync = cpu.sync(*kind, &mut cpu_ram);
        let edge_sync = edge.sync(*kind, &mut edge_ram);
        if cpu_sync.is_err() || edge_sync.is_err() {
            return Err(ParityFailure::Backend {
                case_name,
                phase,
                cpu: cpu_sync,
                edge: edge_sync,
            });
        }
    }

    let cpu_scan = vi.scanout_frame_hash(&cpu_ram);
    let edge_scan = vi.scanout_frame_hash(&edge_ram);
    let (cpu_scanout, edge_scanout) = match (cpu_scan, edge_scan) {
        (Ok(cpu_hash), Ok(edge_hash)) => (cpu_hash, edge_hash),
        (cpu, edge) => {
            return Err(ParityFailure::Scanout {
                case_name,
                cpu,
                edge,
            });
        }
    };

    let (origin, len) = scanout_window(vi);
    let cpu_raw = raw_fb_fnv(&cpu_ram, origin, len);
    let edge_raw = raw_fb_fnv(&edge_ram, origin, len);
    if cpu_scanout != edge_scanout || cpu_raw != edge_raw {
        let (first_byte_diff, first_hidden_diff) =
            first_window_diff(&cpu_ram, &edge_ram, origin, len);
        return Err(ParityFailure::Mismatch(ParityMismatch {
            case_name,
            cpu_scanout,
            edge_scanout,
            cpu_raw_fb: cpu_raw,
            edge_raw_fb: edge_raw,
            first_byte_diff,
            first_hidden_diff,
        }));
    }

    Ok(ParityReceipt {
        scanout_hash: cpu_scanout,
        raw_fb_fnv: cpu_raw,
    })
}

/// [`compare_edgewalker_to_cpu_rdp_lle`], panicking with the full report on failure.
#[must_use = "VI scanout hash and raw framebuffer FNV are the parity receipt"]
pub fn assert_edgewalker_matches_cpu_rdp_lle(
    case_name: &'static str,
    cmds: &[u64],
    syncs: &[RdpSyncKind],
    vi: &Vi,
) -> ParityReceipt {
    compare_edgewalker_to_cpu_rdp_lle(case_name, cmds, syncs, vi)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn sync_phase(kind: RdpSyncKind) -> &'static str {
    match kind {
        RdpSyncKind::Pipe => "sync Pipe",
        RdpSyncKind::Tile => "sync Tile",
        RdpSyncKind::Load => "sync Load",
        RdpSyncKind::Full => "sync Full",
    }
}

/// Origin and byte length covered by [`Vi::scanout_rgba8888`] for this programming.
fn scanout_window(vi: &Vi) -> (u32, u32) {
    let bpp: u32 = match vi.color_type() {
        Ok(ViColorType::Rgba32) => 4,
        Ok(ViColorType::Rgba16) => 2,
        Ok(ViColorType::Off) | Err(_) => 0,
    };
    let width = vi.width & 0x0FFF;
    let height = vi.scanout_height();
    let len = width.saturating_mul(height).saturating_mul(bpp);
    (vi.origin & 0x00FF_FFFF, len)
}

/// FNV-1a 64 over `b"GCFB"`, origin, length, color bytes, then one byte per 9th bit.
fn raw_fb_fnv(rdram: &Rdram, origin: u32, len: u32) -> u64 {
    const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut hash = OFFSET;
    let mut mix = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    };
    for byte in b"GCFB" {
        mix(*byte);
    }
    for byte in origin.to_le_bytes() {
        mix(byte);
    }
    for byte in len.to_le_bytes() {
        mix(byte);
    }
    for i in 0..len {
        let addr = origin.wrapping_add(i);
        mix(rdram.read_u8(addr));
    }
    for i in 0..len {
        let addr = origin.wrapping_add(i);
        mix(u8::from(rdram.read_hidden_bit(addr as usize)));
    }
    hash
}

fn first_window_diff(
    cpu: &Rdram,
    edge: &Rdram,
    origin: u32,
    len: u32,
) -> (Option<u32>, Option<u32>) {
    let mut byte_diff = None;
    let mut hidden_diff = None;
    for i in 0..len {
        let addr = origin.wrapping_add(i);
        if byte_diff.is_none() && cpu.read_u8(addr) != edge.read_u8(addr) {
            byte_diff = Some(addr);
        }
        if hidden_diff.is_none()
            && cpu.read_hidden_bit(addr as usize) != edge.read_hidden_bit(addr as usize)
        {
            hidden_diff = Some(addr);
        }
        if byte_diff.is_some() && hidden_diff.is_some() {
            break;
        }
    }
    (byte_diff, hidden_diff)
}
