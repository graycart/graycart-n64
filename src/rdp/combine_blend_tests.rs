use super::*;
use crate::bus::Rdram;
use crate::rdp::CpuRdpLle;

#[test]
fn sync_full_clears_busy_and_raises_dp() {
    let mut rdp = CpuRdpLle::new();
    rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY | CpuRdpLle::STATUS_CMD_BUSY;
    let mut rdram = Rdram::new(true);
    let action = sync_full(&mut rdp, &mut rdram).unwrap();
    assert_eq!(action, RdpAction::RaiseDpIntr);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_CMD_BUSY, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_GCLK, 0);
}

#[test]
fn set_combine_shell_traps() {
    let err = unimplemented_combine(0x3C, 0x3C00_0000_0000_0000).unwrap_err();
    assert!(matches!(
        err,
        crate::trap::Trap::UnimplementedRdpCommand { cmd_id: 0x3C, .. }
    ));
}
