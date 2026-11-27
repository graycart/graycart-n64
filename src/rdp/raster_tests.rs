use super::*;

#[test]
fn primitive_shell_traps() {
    let err = unimplemented_primitive(0x36, 0x3600_0000_0000_0000).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));
}
