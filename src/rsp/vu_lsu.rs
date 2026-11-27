//! RSP Vector Unit Load/Store instructions (`LWC2` / `SWC2`: `LBV`..`LTV`, `SBV`..`STV`).
//!
//! Cited: N64brew — Reality Signal Processor (Vector Load/Store)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#[cfg(test)]
#[path = "vu_lsu_tests.rs"]
mod tests;

use crate::bus::Bus;
use crate::trap::Trap;

use super::InterpRsp;

/// Execute `LWC2` (`op == 0x32`) vector loads from 4 KiB wrapping DMEM.
pub fn exec_lwc2(rsp: &mut InterpRsp, bus: &Bus, pc: u16, word: u32) -> Result<(), Trap> {
    let base_reg = ((word >> 21) & 0x1F) as usize;
    let vt = ((word >> 16) & 0x1F) as usize;
    let sub = (word >> 11) & 0x1F;
    let e = ((word >> 7) & 0x0F) as usize;
    let off7 = (((word & 0x7F) as i32) << 25) >> 25; // signed 7-bit offset (-64..=63)

    let base_val = rsp.gpr(base_reg);
    let shift = match sub {
        0x00 => 0, // LBV
        0x01 => 1, // LSV
        0x02 => 2, // LLV
        0x03 => 3, // LDV
        0x04 => 4, // LQV
        0x05 => 4, // LRV
        0x06 => 3, // LPV
        0x07 => 3, // LUV
        0x08 => 4, // LHV
        0x09 => 4, // LFV
        0x0B => 4, // LTV
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc: u64::from(pc),
                word,
            })
        }
    };

    let addr = base_val.wrapping_add((off7 << shift) as u32);

    match sub {
        0x00 => {
            // LBV: addr = base + (off << 0). Read 1 byte, write to VPR[vt][byte e]
            let b = rsp.read_dmem_u8(bus, addr);
            rsp.vu.set_byte(vt, e, b);
        }
        0x01 => {
            // LSV: addr = base + (off << 1). Copy 2 bytes (i in 0..2, if e + i < 16)
            let end = (e + 2).min(16);
            for i in 0..(end - e) {
                let b = rsp.read_dmem_u8(bus, addr.wrapping_add(i as u32));
                rsp.vu.set_byte(vt, e + i, b);
            }
        }
        0x02 => {
            // LLV: addr = base + (off << 2). Copy up to 4 bytes (i in 0..4, if e + i < 16)
            let end = (e + 4).min(16);
            for i in 0..(end - e) {
                let b = rsp.read_dmem_u8(bus, addr.wrapping_add(i as u32));
                rsp.vu.set_byte(vt, e + i, b);
            }
        }
        0x03 => {
            // LDV: addr = base + (off << 3). Copy up to 8 bytes (i in 0..8, if e + i < 16)
            let end = (e + 8).min(16);
            for i in 0..(end - e) {
                let b = rsp.read_dmem_u8(bus, addr.wrapping_add(i as u32));
                rsp.vu.set_byte(vt, e + i, b);
            }
        }
        0x04 => {
            // LQV: load from addr up to next 16-byte DMEM boundary
            let count = (16 - (addr & 15) as usize).min(16 - e);
            for i in 0..count {
                let b = rsp.read_dmem_u8(bus, addr.wrapping_add(i as u32));
                rsp.vu.set_byte(vt, e + i, b);
            }
        }
        0x05 => {
            // LRV: load rest of vector from aligned 16-byte start
            let start = 16 - (addr & 15) as usize;
            let aligned = addr & !15;
            let base_byte = start + e;
            for byte_pos in base_byte..16 {
                let b = rsp.read_dmem_u8(bus, aligned.wrapping_add((byte_pos - base_byte) as u32));
                rsp.vu.set_byte(vt, byte_pos, b);
            }
        }
        0x06 => {
            // LPV: packed load (unsigned 8-bit << 8)
            let off_in = (addr & 7) as usize;
            let base_a = addr & !7;
            for i in 0..8 {
                let b = rsp.read_dmem_u8(bus, base_a.wrapping_add(((off_in + i) & 15) as u32));
                rsp.vu.vpr[vt][i] = u16::from(b) << 8;
            }
        }
        0x07 => {
            // LUV: unpacked load (unsigned 8-bit << 7)
            for i in 0..8 {
                let b = rsp.read_dmem_u8(bus, addr.wrapping_add(i as u32));
                rsp.vu.vpr[vt][i] = u16::from(b) << 7;
            }
        }
        0x08 => {
            // LHV: half-packed load (every 2nd byte << 7)
            let base_a = addr & !7;
            let off_a = (addr & 7) as usize;
            for i in 0..8 {
                let b = rsp.read_dmem_u8(bus, base_a.wrapping_add(((off_a + i * 2) & 15) as u32));
                rsp.vu.vpr[vt][i] = u16::from(b) << 7;
            }
        }
        0x09 => {
            // LFV: fourth-packed load (4 elements stepping by 4 bytes)
            let base_a = addr & !7;
            let off_a = (addr & 7) as usize;
            let start = if e < 8 { 0 } else { 4 };
            for k in 0..4 {
                let b = rsp.read_dmem_u8(bus, base_a.wrapping_add(((off_a + k * 4) & 15) as u32));
                rsp.vu.vpr[vt][start + k] = u16::from(b) << 7;
            }
        }
        0x0B => {
            // LTV: transpose load into lane `row = e >> 1` across 8 rotated registers
            // `vpr[base_vr | ((i + row) & 7)][row] = DMEM[aligned + (((addr & 7) + i*2) & 15)]`.
            let base_vr = vt & !7;
            let aligned = addr & !7;
            let row = (e >> 1) & 7;
            for i in 0..8 {
                let vr = base_vr | ((i + row) & 7);
                let src_off = ((addr as usize & 7) + i * 2) & 15;
                let b0 = rsp.read_dmem_u8(bus, aligned.wrapping_add(src_off as u32));
                let b1 = rsp.read_dmem_u8(bus, aligned.wrapping_add(((src_off + 1) & 15) as u32));
                rsp.vu.vpr[vr][row] = u16::from_be_bytes([b0, b1]);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Execute `SWC2` (`op == 0x3A`) vector stores to 4 KiB wrapping DMEM.
pub fn exec_swc2(rsp: &mut InterpRsp, bus: &mut Bus, pc: u16, word: u32) -> Result<(), Trap> {
    let base_reg = ((word >> 21) & 0x1F) as usize;
    let vt = ((word >> 16) & 0x1F) as usize;
    let sub = (word >> 11) & 0x1F;
    let e = ((word >> 7) & 0x0F) as usize;
    let off7 = (((word & 0x7F) as i32) << 25) >> 25; // signed 7-bit offset (-64..=63)

    let base_val = rsp.gpr(base_reg);
    let shift = match sub {
        0x00 => 0, // SBV
        0x01 => 1, // SSV
        0x02 => 2, // SLV
        0x03 => 3, // SDV
        0x04 => 4, // SQV
        0x05 => 4, // SRV
        0x06 => 3, // SPV
        0x07 => 3, // SUV
        0x08 => 4, // SHV
        0x09 => 4, // SFV
        0x0A => 4, // SWV
        0x0B => 4, // STV
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc: u64::from(pc),
                word,
            })
        }
    };

    let addr = base_val.wrapping_add((off7 << shift) as u32);

    match sub {
        0x00 => {
            // SBV: write 1 byte VPR[vt][byte e] to DMEM[addr]
            let b = rsp.vu.get_byte(vt, e);
            rsp.write_dmem_u8(bus, addr, b);
        }
        0x01 => {
            // SSV: write 2 bytes VPR[vt][byte (e + i) & 15] to DMEM[addr + i]
            for i in 0..2 {
                let b = rsp.vu.get_byte(vt, e + i);
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x02 => {
            // SLV: write 4 bytes VPR[vt][byte (e + i) & 15] to DMEM[addr + i]
            for i in 0..4 {
                let b = rsp.vu.get_byte(vt, e + i);
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x03 => {
            // SDV: write 8 bytes VPR[vt][byte (e + i) & 15] to DMEM[addr + i]
            for i in 0..8 {
                let b = rsp.vu.get_byte(vt, e + i);
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x04 => {
            // SQV: store from addr up to next 16-byte DMEM boundary
            let count = 16 - (addr & 15) as usize;
            for i in 0..count {
                let b = rsp.vu.get_byte(vt, e + i);
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x05 => {
            // SRV: store rest of vector to aligned 16-byte start
            let count = (addr & 15) as usize;
            let aligned = addr & !15;
            let start_byte = 16 - count;
            for i in 0..count {
                let b = rsp.vu.get_byte(vt, e + start_byte + i);
                rsp.write_dmem_u8(bus, aligned.wrapping_add(i as u32), b);
            }
        }
        0x06 => {
            // SPV: packed store
            for i in 0..8 {
                let elem = (i + e) & 15;
                let b = if elem < 8 {
                    (rsp.vu.vpr[vt][elem] >> 8) as u8
                } else {
                    (rsp.vu.vpr[vt][elem & 7] >> 7) as u8
                };
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x07 => {
            // SUV: unpacked store
            for i in 0..8 {
                let elem = (i + e) & 15;
                let b = if elem < 8 {
                    (rsp.vu.vpr[vt][elem] >> 7) as u8
                } else {
                    (rsp.vu.vpr[vt][elem & 7] >> 8) as u8
                };
                rsp.write_dmem_u8(bus, addr.wrapping_add(i as u32), b);
            }
        }
        0x08 => {
            // SHV: half-packed store
            let base_a = addr & !7;
            let off_a = (addr & 7) as usize;
            for i in 0..8 {
                let b0 = rsp.vu.get_byte(vt, e + i * 2);
                let b1 = rsp.vu.get_byte(vt, e + i * 2 + 1);
                let val = (b0 << 1) | (b1 >> 7);
                rsp.write_dmem_u8(bus, base_a.wrapping_add(((off_a + i * 2) & 15) as u32), val);
            }
        }
        0x09 => {
            // SFV: fourth-packed store
            let base_a = addr & !7;
            let off_a = (addr & 7) as usize;
            let lanes: &[usize] = match e {
                0 => &[0, 1, 2, 3],
                8 => &[4, 5, 6, 7],
                _ => &[0, 0, 0, 0],
            };
            for (k, &lane) in lanes.iter().enumerate() {
                let b = (rsp.vu.vpr[vt][lane] >> 7) as u8;
                rsp.write_dmem_u8(bus, base_a.wrapping_add(((off_a + k * 4) & 15) as u32), b);
            }
        }
        0x0A => {
            // SWV: wrapped 16-byte store
            let base_a = addr & !7;
            let off_a = (addr & 7) as usize;
            for i in 0..16 {
                let b = rsp.vu.get_byte(vt, e + i);
                rsp.write_dmem_u8(bus, base_a.wrapping_add(((off_a + i) & 15) as u32), b);
            }
        }
        0x0B => {
            // STV: transpose store reading diagonal `vpr[base_vr | ((i + col) & 7)][i]`
            // so 8× LTV followed by 8× STV produces an exact 8×8 matrix transpose.
            let base_vr = vt & !7;
            let aligned = addr & !7;
            let col = (e >> 1) & 7;
            for i in 0..8 {
                let vr = base_vr | ((i + col) & 7);
                let half = rsp.vu.vpr[vr][i];
                let bytes = half.to_be_bytes();
                let dst_off = ((addr as usize & 7) + i * 2) & 15;
                rsp.write_dmem_u8(bus, aligned.wrapping_add(dst_off as u32), bytes[0]);
                rsp.write_dmem_u8(
                    bus,
                    aligned.wrapping_add(((dst_off + 1) & 15) as u32),
                    bytes[1],
                );
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
