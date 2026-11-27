//! Reality Signal Processor (RSP) Vector Unit Compare, Clip, Merge, Logic, and Move Unit Tests.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

use crate::rsp::vu_logic_cmp::exec_vu_logic_cmp;
use crate::rsp::InterpRsp;
use crate::trap::Trap;

fn enc_vu(e: u32, vt: u32, vs: u32, vd: u32, funct: u32) -> u32 {
    (0x12 << 26) | (1 << 25) | (e << 21) | (vt << 16) | (vs << 11) | (vd << 6) | funct
}

#[test]
fn test_vlt_veq_vne_vge_and_vco_tiebreak() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[1] = [10, 20, 30, 0xFFFF, 50, 60, 70, 80];
    rsp.vu.vpr[2] = [20, 20, 25, 0x0000, 50, 60, 70, 80];

    // VLT: lane 0 (10<20), lane 3 (-1<0); plus set VCO ne+carry on lane 1 so equal lane 1 is <
    rsp.vu.vco = (1 << 1) | (1 << 9);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x20)).unwrap();
    assert_eq!(rsp.vu.vcc, (1 << 0) | (1 << 1) | (1 << 3));
    assert_eq!(rsp.vu.vco, 0);
    assert_eq!(rsp.vu.vpr[3][0], 10);
    assert_eq!(rsp.vu.vpr[3][2], 25);

    // VEQ: lane 1, 4, 5, 6, 7 equal; mark lane 4 NOT-equal in VCO -> lane 4 excluded
    rsp.vu.vco = 1 << 12;
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x21)).unwrap();
    assert_eq!(rsp.vu.vcc, (1 << 1) | (1 << 5) | (1 << 6) | (1 << 7));
    assert_eq!(rsp.vu.vco, 0);

    // VNE: lanes 0, 2, 3 not equal
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x22)).unwrap();
    assert_eq!(rsp.vu.vcc, (1 << 0) | (1 << 2) | (1 << 3));

    // VGE: lanes 1, 2, 4, 5, 6, 7 >=
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x23)).unwrap();
    assert_eq!(rsp.vu.vcc, 0b1111_0110);
}

#[test]
fn test_vch_vcl_vcr_clipping() {
    let mut rsp = InterpRsp::new();
    // Lane 0: +100 vs +50 -> clipped to +50 (GE=1)
    // Lane 1: -100 vs +50 -> clipped to -50 (LE=1)
    // Lane 2: -51 vs +50 -> diff = -1 -> VCE=1, LE=1, eq=true
    rsp.vu.vpr[1] = [100, (-100i16) as u16, (-51i16) as u16, 10, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [50, 50, 50, 50, 0, 0, 0, 0];

    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x25)).unwrap(); // VCH
    assert_eq!(rsp.vu.vpr[3][0], 50);
    assert_eq!(rsp.vu.vpr[3][1], (-50i16) as u16);
    assert_eq!(rsp.vu.vpr[3][3], 10);
    assert_ne!(rsp.vu.vce & (1 << 2), 0); // lane 2 VCE set

    // Follow with VCL on low halves (clears VCO & VCE)
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 4, 0x24)).unwrap(); // VCL
    assert_eq!(rsp.vu.vco, 0);
    assert_eq!(rsp.vu.vce, 0);

    // VCR single-precision 1's-complement clip
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 5, 0x26)).unwrap();
    assert_eq!(rsp.vu.vpr[5][0], 50);
    assert_eq!(rsp.vu.vpr[5][1], !50u16);
}

#[test]
fn test_vmrg_and_all_bitwise_logic() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[1] = [0xF0F0; 8];
    rsp.vu.vpr[2] = [0x0FF0; 8];
    rsp.vu.vcc = 0x000F;

    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x27)).unwrap(); // VMRG
    assert_eq!(rsp.vu.vpr[3][0], 0xF0F0);
    assert_eq!(rsp.vu.vpr[3][7], 0x0FF0);

    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x28)).unwrap(); // VAND
    assert_eq!(rsp.vu.vpr[3][0], 0x00F0);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x29)).unwrap(); // VNAND
    assert_eq!(rsp.vu.vpr[3][0], !0x00F0u16);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x2A)).unwrap(); // VOR
    assert_eq!(rsp.vu.vpr[3][0], 0xFFF0);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x2B)).unwrap(); // VNOR
    assert_eq!(rsp.vu.vpr[3][0], !0xFFF0u16);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x2C)).unwrap(); // VXOR
    assert_eq!(rsp.vu.vpr[3][0], 0xFF00);
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x2D)).unwrap(); // VNXOR
    assert_eq!(rsp.vu.vpr[3][0], !0xFF00u16);
}

#[test]
fn test_vmov_single_lane_preservation_and_nop() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[2] = [10, 20, 30, 40, 50, 60, 70, 80];
    rsp.vu.vpr[3] = [0xAA; 8];

    // VMOV v3[2], v2[5] (e = 8+5 = 13 broadcast lane 5, vs = 2 -> de = 2)
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(13, 2, 2, 3, 0x33)).unwrap();
    assert_eq!(
        rsp.vu.vpr[3],
        [0xAA, 0xAA, 60, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]
    );
    assert_eq!(rsp.vu.acc_lo, [60; 8]);

    // VNOP does not disturb v3
    exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 2, 3, 0x37)).unwrap();
    assert_eq!(rsp.vu.vpr[3][2], 60);

    // Unknown funct traps
    let res = exec_vu_logic_cmp(&mut rsp, 0, enc_vu(0, 2, 0, 3, 0x30));
    assert!(matches!(res, Err(Trap::UnimplementedOpcode { .. })));
}
