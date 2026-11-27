//! ComputeDevice open and buffer round-trip.
//!
//! A machine with no compute adapter must trap. A machine with one must
//! upload and read back the same bytes.
//!
//! Cited: wgpu 24.0.5 — Queue::write_buffer, Buffer::map_async
//!   https://docs.rs/wgpu/24.0.5/wgpu/
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor

use crate::trap::Trap;

use super::super::EdgeWalker;
use super::ComputeDevice;

fn expect_edgewalker_trap(err: Trap) {
    assert_eq!(EdgeWalker::COMPUTE_DISPATCH, 0x0001);
    assert_eq!(
        err,
        Trap::UnimplementedRegister {
            device: "EdgeWalker",
            offset: 0x0001,
            is_write: true,
        }
    );
}

#[test]
fn open_succeeds_or_traps_cleanly() {
    match ComputeDevice::open() {
        Ok(device) => {
            let info = device.adapter_info();
            eprintln!(
                "ComputeDevice::open Ok backend={:?} name={} type={:?}",
                info.backend, info.name, info.device_type
            );
            assert!(!info.name.is_empty());
            assert!(device.device().limits().max_compute_workgroup_size_x >= 1);
            let pipeline = device
                .create_compute_pipeline("@compute @workgroup_size(1)\nfn main() {}", "main")
                .expect("opened device accepts a compute pipeline");
            device
                .dispatch_workgroups(&pipeline, None, 1, 1, 1)
                .expect("opened device accepts a compute dispatch");
            let buffer = device
                .create_storage_buffer(16)
                .expect("opened device accepts a storage buffer");
            assert_eq!(buffer.size(), 16);
        }
        Err(err) => {
            eprintln!("ComputeDevice::open trapped: {err}");
            expect_edgewalker_trap(err);
        }
    }
}

#[test]
fn buffer_roundtrip_when_device_available() {
    let Ok(device) = ComputeDevice::open() else {
        eprintln!("ComputeDevice::open unavailable; skipping buffer roundtrip");
        return;
    };
    let payload: [u8; 32] = [
        0x00, 0x7F, 0x80, 0xFF, 0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x01, 0x23, 0x45,
        0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA,
        0xBB, 0xCC,
    ];
    let buffer = device
        .create_storage_buffer(payload.len() as u64)
        .expect("storage buffer");
    device
        .write_buffer(&buffer, 0, &payload)
        .expect("write_buffer");
    let back = device.readback(&buffer).expect("readback");
    assert_eq!(back, payload);
    eprintln!(
        "ComputeDevice buffer roundtrip Ok ({} bytes, {})",
        back.len(),
        device.adapter_info().name
    );
}
