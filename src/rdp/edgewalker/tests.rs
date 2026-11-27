//! EdgeWalker W0 seam: CPU parity with [`CpuRdpLle`] and a trapping compute dispatch.
//!
//! **M6 gate:** bit-exact frame hash EdgeWalker == CpuRdpLle.

use serde_json;

use crate::bus::Rdram;
use crate::rdp::{CpuRdpLle, RdpBackend, RdpSyncKind};
use crate::trap::Trap;

use super::EdgeWalker;

fn fill_rect_cmds() -> [u64; 5] {
    // RGBA16, 16 pixels wide, DRAM address 0. Same list as
    // `submit_fill_rectangle_writes_known_pixel`.
    let color = (0x3Fu64 << 56) | (2 << 51) | (15 << 32);
    let modes = (0x2Fu64 << 56) | (3 << 52) | (3 << 38);
    let fill = (0x37u64 << 56) | 0x1111_2222;
    let scissor = (0x2Du64 << 56) | (4 << 12) | 4;
    let rect = (0x36u64 << 56) | (4 << 44);
    [color, modes, fill, scissor, rect]
}

fn assert_ram_window_eq(left: &Rdram, right: &Rdram) {
    assert_eq!(&left.as_slice()[..16], &right.as_slice()[..16]);
    for addr in 0..16 {
        assert_eq!(left.read_hidden_bit(addr), right.read_hidden_bit(addr));
    }
}

#[test]
fn submit_fill_matches_cpu_rdp_lle_bytes_and_state() {
    let cmds = fill_rect_cmds();
    let mut edge = EdgeWalker::new();
    let mut cpu = CpuRdpLle::new();
    let mut edge_ram = Rdram::new(false);
    let mut cpu_ram = Rdram::new(false);

    edge.submit(&cmds, &mut edge_ram).unwrap();
    cpu.submit(&cmds, &mut cpu_ram).unwrap();

    assert_eq!(edge_ram.read_u16(0).unwrap(), 0x1111);
    assert_eq!(edge_ram.read_u16(2).unwrap(), 0x2222);
    assert_eq!(edge_ram.read_u16(4).unwrap(), 0);
    assert_ram_window_eq(&edge_ram, &cpu_ram);

    for kind in [
        RdpSyncKind::Pipe,
        RdpSyncKind::Tile,
        RdpSyncKind::Load,
        RdpSyncKind::Full,
    ] {
        edge.sync(kind, &mut edge_ram).unwrap();
        cpu.sync(kind, &mut cpu_ram).unwrap();
    }
    assert_ram_window_eq(&edge_ram, &cpu_ram);

    let edge_json = serde_json::to_value(&edge).unwrap();
    let cpu_json = serde_json::to_value(&cpu).unwrap();
    assert_eq!(edge_json["parity"], cpu_json);
}

#[test]
fn unimplemented_command_traps_like_cpu_rdp_lle() {
    let word = 0x0Fu64 << 56;
    let mut edge = EdgeWalker::new();
    let mut cpu = CpuRdpLle::new();
    let mut edge_ram = Rdram::new(false);
    let mut cpu_ram = Rdram::new(false);

    let edge_err = edge.submit(&[word], &mut edge_ram).unwrap_err();
    let cpu_err = cpu.submit(&[word], &mut cpu_ram).unwrap_err();
    assert_eq!(edge_err, cpu_err);
    assert!(matches!(
        edge_err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x0F, .. }
    ));
    assert_eq!(&edge_ram.as_slice()[..8], &cpu_ram.as_slice()[..8]);
}

#[test]
fn dispatch_compute_traps_until_shaders_land() {
    let edge = EdgeWalker::new();
    let err = edge.dispatch_compute().unwrap_err();
    assert_eq!(
        err,
        Trap::UnimplementedRegister {
            device: "EdgeWalker",
            offset: EdgeWalker::COMPUTE_DISPATCH,
            is_write: true,
        }
    );
    assert!(err.to_string().contains("EdgeWalker"));
}

#[test]
fn serde_round_trip_preserves_parity_state() {
    let mut edge = EdgeWalker::new();
    let mut ram = Rdram::new(false);
    edge.submit(&fill_rect_cmds(), &mut ram).unwrap();
    let json = serde_json::to_string(&edge).unwrap();
    let back: EdgeWalker = serde_json::from_str(&json).unwrap();
    assert_eq!(
        serde_json::to_value(&edge).unwrap(),
        serde_json::to_value(&back).unwrap()
    );
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn compute_device_open_traps_until_shaders_land() {
    let err = super::ComputeDevice::open().unwrap_err();
    assert_eq!(
        err,
        Trap::UnimplementedRegister {
            device: "EdgeWalker",
            offset: EdgeWalker::COMPUTE_DISPATCH,
            is_write: true,
        }
    );
}
