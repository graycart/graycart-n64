//! Seed lists where EdgeWalker (Vulkan compute LLE) matches CpuRdpLle.
//!
//! Cited: N64brew — Reality Display Processor (`Sync Full`, `Fill Rectangle`)
//!   https://n64brew.dev/wiki/Reality_Display_Processor

use crate::rdp::RdpSyncKind;

use super::parity::{
    assert_edgewalker_matches_cpu_rdp_lle, compare_edgewalker_to_cpu_rdp_lle,
    fill_rectangle_smoke_cmds, rgba16_replicate_scanout, ParityFailure, ParityMismatch, SYNC_FULL,
};

fn window() -> crate::vi::Vi {
    rgba16_replicate_scanout(16)
}

#[test]
fn empty_submit_matches_cpu_rdp_lle() {
    let receipt = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &window());
    assert_ne!(receipt.scanout_hash, 0);
    assert_ne!(receipt.raw_fb_fnv, 0);
    assert_ne!(receipt.scanout_hash, receipt.raw_fb_fnv);
}

#[test]
fn sync_full_only_matches_cpu_rdp_lle() {
    let vi = window();
    let submitted =
        assert_edgewalker_matches_cpu_rdp_lle("sync-full-command", &[SYNC_FULL], &[], &vi);
    let via_sync =
        assert_edgewalker_matches_cpu_rdp_lle("sync-full-barrier", &[], &[RdpSyncKind::Full], &vi);
    let blank = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &vi);
    assert_eq!(submitted.scanout_hash, blank.scanout_hash);
    assert_eq!(submitted.raw_fb_fnv, blank.raw_fb_fnv);
    assert_eq!(via_sync.scanout_hash, blank.scanout_hash);
    assert_eq!(via_sync.raw_fb_fnv, blank.raw_fb_fnv);
}

#[test]
fn fill_rectangle_smoke_matches_cpu_rdp_lle() {
    let vi = window();
    let filled = assert_edgewalker_matches_cpu_rdp_lle(
        "fill-rectangle",
        &fill_rectangle_smoke_cmds(),
        &[],
        &vi,
    );
    let blank = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &vi);
    assert_ne!(
        filled.scanout_hash, blank.scanout_hash,
        "FillRectangle must change the VI scanout hash"
    );
    assert_ne!(
        filled.raw_fb_fnv, blank.raw_fb_fnv,
        "FillRectangle must change the raw framebuffer FNV"
    );
}

#[test]
fn mismatch_report_names_both_frame_hashes() {
    let err = ParityFailure::Mismatch(ParityMismatch {
        case_name: "synthetic",
        cpu_scanout: 0x1111_2222_3333_4444,
        edge_scanout: 0xAAAA_BBBB_CCCC_DDDD,
        cpu_raw_fb: 0x0102_0304_0506_0708,
        edge_raw_fb: 0x090A_0B0C_0D0E_0F10,
        first_byte_diff: Some(0x20),
        first_hidden_diff: Some(0x22),
    });
    let text = err.to_string();
    assert!(text.contains("EdgeWalker (Vulkan compute LLE)"), "{text}");
    assert!(text.contains("CpuRdpLle"), "{text}");
    assert!(text.contains("0x1111222233334444"), "{text}");
    assert!(text.contains("0xaaaabbbbccccdddd"), "{text}");
    assert!(text.contains("0x0102030405060708"), "{text}");
    assert!(text.contains("0x090a0b0c0d0e0f10"), "{text}");
    assert!(text.contains("first byte diff"), "{text}");
    assert!(text.contains("first 9th-bit diff"), "{text}");
    assert!(compare_edgewalker_to_cpu_rdp_lle("empty-submit", &[], &[], &window()).is_ok());
}
