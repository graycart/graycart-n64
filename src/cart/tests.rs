//! Unit tests for ROM loader, byte-order normalization, header parsing, and CIC detection.
//! Uses synthetic in-memory headers only — zero real ROMs or proprietary IPL3 dumps.

use super::*;

fn make_synthetic_z64(title: &str, entry_pc: u32) -> Vec<u8> {
    let mut rom = vec![0u8; 0x2000];
    rom[0..4].copy_from_slice(&MAGIC_Z64);
    rom[4..8].copy_from_slice(&0x0000_000Fu32.to_be_bytes());
    rom[8..12].copy_from_slice(&entry_pc.to_be_bytes());
    rom[12..16].copy_from_slice(&0x0000_1444u32.to_be_bytes());
    rom[16..20].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    rom[20..24].copy_from_slice(&0x5566_7788u32.to_be_bytes());
    let title_bytes = title.as_bytes();
    let copy_len = title_bytes.len().min(20);
    rom[0x20..0x20 + copy_len].copy_from_slice(&title_bytes[..copy_len]);
    for b in &mut rom[0x20 + copy_len..0x34] {
        *b = b' ';
    }
    rom[0x3B] = b'N';
    rom[0x3C] = b'G';
    rom[0x3D] = b'C';
    rom[0x3E] = b'E';
    rom[0x3F] = 0x01;
    // Synthetic payload after 0x1000 so endianness round-trip checks code bytes too.
    rom[0x1000..0x1004].copy_from_slice(&0x3C08_8000u32.to_be_bytes());
    rom[0x1004..0x1008].copy_from_slice(&0x2508_1234u32.to_be_bytes());
    rom
}

#[test]
fn canonicalizes_z64_v64_and_n64_to_identical_big_endian_image() {
    let z64 = make_synthetic_z64("GRAYCART TEST", 0x8000_1000);

    let mut v64 = z64.clone();
    for chunk in v64.as_chunks_mut::<2>().0 {
        chunk.swap(0, 1);
    }

    let mut n64 = z64.clone();
    for chunk in n64.as_chunks_mut::<4>().0 {
        chunk.reverse();
    }

    let c_z64 = Cart::from_bytes(&z64).expect("z64 must load");
    let c_v64 = Cart::from_bytes(&v64).expect("v64 must load");
    let c_n64 = Cart::from_bytes(&n64).expect("n64 must load");

    assert_eq!(c_z64.source_format(), RomFormat::Z64);
    assert_eq!(c_v64.source_format(), RomFormat::V64);
    assert_eq!(c_n64.source_format(), RomFormat::N64);

    assert_eq!(c_z64.rom(), c_v64.rom());
    assert_eq!(c_z64.rom(), c_n64.rom());
    assert_eq!(c_z64.header(), c_v64.header());
    assert_eq!(c_z64.header(), c_n64.header());

    let h = c_z64.header();
    assert_eq!(h.pi_dom1_config, 0x8037_1240);
    assert_eq!(h.clock_rate, 0x0000_000F);
    assert_eq!(h.entry_pc, 0x8000_1000);
    assert_eq!(h.release, 0x0000_1444);
    assert_eq!(h.crc1, 0x1122_3344);
    assert_eq!(h.crc2, 0x5566_7788);
    assert_eq!(h.title, "GRAYCART TEST");
    assert_eq!(h.media_format, b'N');
    assert_eq!(h.cart_id, [b'G', b'C']);
    assert_eq!(h.region_code, b'E');
    assert_eq!(h.rom_version, 1);
}

#[test]
fn rejects_too_small_invalid_magic_and_unaligned_swapped_images() {
    assert_eq!(
        Cart::from_bytes(&[0u8; 64]).unwrap_err(),
        RomError::TooSmall(64)
    );

    let mut bad = vec![0u8; MIN_ROM_SIZE];
    bad[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        Cart::from_bytes(&bad).unwrap_err(),
        RomError::UnrecognizedMagic([0xDE, 0xAD, 0xBE, 0xEF])
    );

    let mut odd_v64 = vec![0u8; MIN_ROM_SIZE + 1];
    odd_v64[0..4].copy_from_slice(&MAGIC_V64);
    assert_eq!(
        Cart::from_bytes(&odd_v64).unwrap_err(),
        RomError::UnalignedLength {
            format: RomFormat::V64,
            len: MIN_ROM_SIZE + 1,
        }
    );

    let mut unaligned_n64 = vec![0u8; MIN_ROM_SIZE + 2];
    unaligned_n64[0..4].copy_from_slice(&MAGIC_N64);
    assert_eq!(
        Cart::from_bytes(&unaligned_n64).unwrap_err(),
        RomError::UnalignedLength {
            format: RomFormat::N64,
            len: MIN_ROM_SIZE + 2,
        }
    );
}

#[test]
fn ieee_crc32_and_cic_classification_match_standard_vectors() {
    // Standard IEEE 802.3 CRC32 check value for ASCII "123456789".
    assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);

    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_6101),
        CicType::Cic6101
    );
    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_6102),
        CicType::Cic6102
    );
    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_6103),
        CicType::Cic6103
    );
    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_6105),
        CicType::Cic6105
    );
    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_6106),
        CicType::Cic6106
    );
    assert_eq!(
        CicType::from_ipl3_crc32(CicType::CRC32_7102),
        CicType::Cic7102
    );
    assert_eq!(
        CicType::from_ipl3_crc32(0x1234_5678),
        CicType::Unknown(0x1234_5678)
    );

    assert_eq!(CicType::Cic6102.boot_seed(), 0x3F);
    assert_eq!(CicType::Cic6103.boot_seed(), 0x78);
    assert_eq!(CicType::Cic6105.boot_seed(), 0x91);
    assert_eq!(CicType::Cic6106.boot_seed(), 0x85);
}
