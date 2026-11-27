// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! EdgeWalker submit / sync: compute trap without `vulkan-rdp`, GPU parity with it.
//!
//! **M6 gate:** bit-exact frame hash EdgeWalker == CpuRdpLle when the span
//! shader runs. The software RDP is not on the EdgeWalker hot path.

use crate::bus::Rdram;
#[cfg(feature = "vulkan-rdp")]
use crate::rdp::CpuRdpLle;
use crate::rdp::{RdpBackend, RdpSyncKind};
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

#[cfg(feature = "vulkan-rdp")]
fn assert_ram_window_eq(left: &Rdram, right: &Rdram) {
    assert_eq!(&left.as_slice()[..16], &right.as_slice()[..16]);
    for addr in 0..16 {
        assert_eq!(left.read_hidden_bit(addr), right.read_hidden_bit(addr));
    }
}

#[cfg(not(feature = "vulkan-rdp"))]
fn assert_compute_unavailable(err: Trap) {
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

#[cfg(not(feature = "vulkan-rdp"))]
#[test]
fn submit_and_sync_trap_without_vulkan_rdp() {
    let mut edge = EdgeWalker::new();
    let mut ram = Rdram::new(false);
    let submit_err = edge.submit(&fill_rect_cmds(), &mut ram).unwrap_err();
    assert_compute_unavailable(submit_err);
    assert!(ram.as_slice()[..16].iter().all(|byte| *byte == 0));
    for kind in [
        RdpSyncKind::Pipe,
        RdpSyncKind::Tile,
        RdpSyncKind::Load,
        RdpSyncKind::Full,
    ] {
        let err = edge.sync(kind, &mut ram).unwrap_err();
        assert_compute_unavailable(err);
    }
    assert!(ram.as_slice()[..16].iter().all(|byte| *byte == 0));
}

#[cfg(feature = "vulkan-rdp")]
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
    assert!(edge_json.get("parity").is_none());
    assert!(edge_json.get("tmem").is_none());
    assert_eq!(edge_json["fill_color"], serde_json::json!(0x1111_2222u32));
    assert_eq!(edge_json["color_image"]["size"], 2);
    assert_eq!(edge_json["color_image"]["width"], 15);
    assert_eq!(edge_json["scissor"]["lrx"], 4);
    assert_eq!(edge_json["scissor"]["lry"], 4);
}

#[cfg(feature = "vulkan-rdp")]
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

#[cfg(not(feature = "vulkan-rdp"))]
#[test]
fn unimplemented_command_traps_compute_unavailable() {
    let word = 0x0Fu64 << 56;
    let mut edge = EdgeWalker::new();
    let mut ram = Rdram::new(false);
    assert_compute_unavailable(edge.submit(&[word], &mut ram).unwrap_err());
}

#[cfg(not(feature = "vulkan-rdp"))]
#[test]
fn dispatch_compute_traps_without_vulkan_rdp() {
    let mut edge = EdgeWalker::new();
    assert_compute_unavailable(edge.dispatch_compute().unwrap_err());
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn dispatch_compute_runs_ingest_stage() {
    EdgeWalker::new()
        .dispatch_compute()
        .expect("ingest stage on an open EdgeWalker device");
}

#[test]
fn serde_round_trip_preserves_latched_state() {
    #[cfg(feature = "vulkan-rdp")]
    let edge = {
        let mut latched = EdgeWalker::new();
        let mut ram = Rdram::new(false);
        latched.submit(&fill_rect_cmds(), &mut ram).unwrap();
        latched
    };
    #[cfg(not(feature = "vulkan-rdp"))]
    let edge = EdgeWalker::new();
    let json = serde_json::to_string(&edge).unwrap();
    let back: EdgeWalker = serde_json::from_str(&json).unwrap();
    assert_eq!(
        serde_json::to_value(&edge).unwrap(),
        serde_json::to_value(&back).unwrap()
    );
    let value = serde_json::to_value(&edge).unwrap();
    assert!(value.get("parity").is_none());
    assert!(value.get("tmem").is_none());
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn compute_device_open_succeeds_when_gpu_present() {
    let device = super::ComputeDevice::open().expect("EdgeWalker compute device");
    assert!(!device.adapter_info().name.is_empty());
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn edgewalker_reuses_persistent_gpu_state_across_submits() {
    let cmds = fill_rect_cmds();
    let mut edge = EdgeWalker::new();
    let mut ram = Rdram::new(false);

    edge.submit(&cmds, &mut ram).unwrap();
    assert!(edge.gpu.is_some());
    let arc1 = edge.gpu.as_ref().unwrap().clone();

    // Second submit with a different fill color and rect target
    let color2 = (0x37u64 << 56) | 0x3333_4444;
    let rect2 = (0x36u64 << 56) | (8 << 44) | (8 << 32);
    edge.submit(&[color2, rect2], &mut ram).unwrap();

    assert!(edge.gpu.is_some());
    let arc2 = edge.gpu.as_ref().unwrap().clone();
    assert!(std::sync::Arc::ptr_eq(&arc1, &arc2));

    // Verify serde round trip drops GPU handle cleanly
    let json = serde_json::to_string(&edge).unwrap();
    let back: EdgeWalker = serde_json::from_str(&json).unwrap();
    assert!(back.gpu.is_none());
}
