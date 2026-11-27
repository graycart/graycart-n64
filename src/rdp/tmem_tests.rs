use super::*;
use crate::rdp::CpuRdpLle;

#[test]
fn clear_tmem_zeros_buffer() {
    let mut rdp = CpuRdpLle::new();
    rdp.tmem[0] = 0xFF;
    clear_tmem(&mut rdp);
    assert!(rdp.tmem.iter().all(|&b| b == 0));
}
