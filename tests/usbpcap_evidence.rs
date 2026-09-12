//! Golden checks for the controlled USBPcap2 captures.
//!
//! The parser is intentionally small and read-only.  It validates the
//! USBPcap envelope used by the evidence rather than depending on a local
//! Wireshark/tshark installation, so a protocol-table change cannot silently
//! forget the captured 64-byte report or the UI-correlated one-byte deltas.
//! Both pcapng (the Apply captures) and classic pcap (the resident capture)
//! are accepted.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use redsamurai_config::device_apply::{ApplyError, ApplyPlan};
use redsamurai_config::device_protocol::{
    APPLY_LIGHT_MODE_FRAME_INDEX, APPLY_SEQUENCE_FRAME_COUNT,
};
use redsamurai_config::profile::Profile;

const TARGET_VID_PID_DESCRIPTOR: [u8; 12] = [
    0x12, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x08, 0xD9, 0x04, 0x55, 0xFC,
];

const CLASSIC_EVIDENCE_FIXTURE: &str = "hid_reenum_usbpcap2_20260909_131359_ca7d7c9e.pcap";
const RECONNECT_EVIDENCE_FIXTURE: &str =
    "reconnect-pnp-20260909T063849634Z/reconnect-pnp-usbpcap2.pcap";
const RECONNECT_EVIDENCE_BYTES: usize = 80_596;
const RECONNECT_EVIDENCE_RECORDS: usize = 1_712;
const CONTROLLED_RUN_DIR: &str = "controlled-polling-apply-20260909T065648206Z-f6b0cfc8";
const RESTART_TRACE_FIXTURE: &str =
    "controlled-polling-apply-20260909T065648206Z-f6b0cfc8/restart-trace-usbpcap2.pcap";
const RESTART_TRACE_BYTES: usize = 367_062;
const RESTART_TRACE_RECORDS: usize = 3_772;
const RESTART_TARGET_PACKETS: usize = 806;
const RESTART_SET_REPORTS: usize = 195;
const RESTART_GET_REPORTS: usize = 193;
const HARDWARE_EVIDENCE_DIR: &str = "hardware-evidence-20260909T071720685Z-5d9fdff2";
const HARDWARE_RECONNECT_FIXTURE: &str =
    "hardware-evidence-20260909T071720685Z-5d9fdff2/reconnect-and-apply.pcap";
const HARDWARE_RECONNECT_BYTES: usize = 3_053_924;
const HARDWARE_RECONNECT_RECORDS: usize = 29_246;
const HARDWARE_RECONNECT_SHA256: &str =
    "551787b5555adb3ed49db90e63db3c8f81ce97460b701ed88d0570cd19e61f10";
const HARDWARE_RECONNECT_TARGET_PACKETS: usize = 2_010;
const HARDWARE_RECONNECT_TARGET_CONTROLS: usize = 1_976;
const HARDWARE_RECONNECT_SET_REPORTS: usize = 546;
const HARDWARE_RECONNECT_GET_REPORTS: usize = 386;
const HARDWARE_RECONNECT_DESCRIPTOR_COUNT: usize = 2;
const HARDWARE_RECONNECT_APPLY_SET_START: usize = 390;
const HARDWARE_APPLY_SET_REPORTS: usize = 156;
const HARDWARE_RESTART_FIXTURE: &str =
    "hardware-evidence-20260909T071720685Z-5d9fdff2/restart-readback-usbpcap2.pcap";
const HARDWARE_RESTART_BYTES: usize = 277_014;
const HARDWARE_RESTART_RECORDS: usize = 2_762;
const HARDWARE_RESTART_SHA256: &str =
    "a47c835e21fc5b8964f92a64bfca80abd8bdd40a4a05ad2668675f4e7414c8a3";
const HARDWARE_RESTART_TARGET_PACKETS: usize = 806;
const HARDWARE_RESTART_TARGET_CONTROLS: usize = 806;
const HARDWARE_RESTART_SET_REPORTS: usize = 195;
const HARDWARE_RESTART_GET_REPORTS: usize = 193;
const HARDWARE_RESTART_DESCRIPTOR_COUNT: usize = 1;
const HARDWARE_PROFILE_BEFORE_METADATA: &str = "profile-before-ui.txt";
const HARDWARE_PROFILE_AFTER_METADATA: &str = "profile-after-ui.txt";
const HARDWARE_RESTART_STATE: &str = "restart-readback-state.json";
const UI_APPLY_TRACE_FIXTURE: &str =
    "controlled-polling-apply-20260909T065648206Z-f6b0cfc8/ui-apply-trace-usbpcap2.pcap";
const UI_APPLY_TRACE_BYTES: usize = 4_020_271;
const UI_APPLY_TRACE_RECORDS: usize = 38_323;
const UI_APPLY_TARGET_PACKETS: usize = 318;
const UI_APPLY_SET_REPORTS: usize = 156;
const LIGHT_RAINBOW_FIXTURE: &str = "official-light-rainbow-20260912/root2.pcap";
const LIGHT_RAINBOW_BYTES: usize = 2_203_209;
const LIGHT_RAINBOW_SHA256: &str =
    "142cc8292b91c95653bbfc7fce9a01dc9069c2411a35f3fb5ca4d14f2702cfd3";
const PROFILE_EVIDENCE_DIR: &str = "rs-observation-20260909T061712713Z-af8fbf35";
const PROFILE_BEFORE: &str = "RSProfile1.pfd.pre-action.bak";
const PROFILE_AFTER: &str = "RSProfile1.pfd.after-apply.copy";
const PROFILE_BYTES: usize = 21_672;
const PROFILE_HASH_125_HZ: &str =
    "3acbf08cf550cbd1ede29f177a9e383965b66aa22bb82bca7215634db4fe14b5";
const PROFILE_HASH_250_HZ: &str =
    "cfdcededd6f5060cdbc2555588e62027bc36e1904348c7a2eb575aac827ce1ca";
const POLLING_RATE_READBACK_125_HZ: &str = "020832600500fafa0800080002";
const POLLING_RATE_READBACK_250_HZ: &str = "020832600500fafa0400080002";

const ENUMERATION_CONTROL_PAYLOADS: &[&str] = &[
    "8006000100001200",
    "1201000200000008d90455fc100200020001",
    "8006000200005400",
    "09025400030100a03209040000010301020009211001000122430007058103080001090401000103010100092110010001222f0007058203080002090402000103000000092110010001226a0007058303080004",
    "0009010000000000",
    "",
];

const HARDWARE_ENUMERATION_CONTROL_PAYLOADS: &[&str] = &[
    "8006000100001200",
    "1201000200000008d90455fc100200020001",
    "8006000200000900",
    "09025400030100a032",
    "8006000200005400",
    "09025400030100a03209040000010301020009211001000122430007058103080001090401000103010100092110010001222f0007058203080002090402000103000000092110010001226a0007058303080004",
    "0009010000000000",
    "",
];

const PCAPNG_EVIDENCE_FIXTURES: &[(&str, usize, usize)] = &[
    ("polling_rate_1000_to_250_usbpcap2.pcapng", 155, 1),
    ("polling_rate_250_to_1000_usbpcap2.pcapng", 155, 1),
    ("polling_rate_1000_to_500_usbpcap2.pcapng", 155, 1),
    ("polling_rate_500_to_125_usbpcap2.pcapng", 155, 1),
    ("ui_light_batch_usbpcap2.pcapng", 859, 5),
    ("dpi_batch_usbpcap2.pcapng", 620, 4),
    ("general_button_usbpcap2.pcapng", 620, 4),
    ("readback_apply_correct_usbpcap2.pcapng", 315, 2),
    ("readback_uacoff_apply_usbpcap2.pcapng", 155, 1),
    ("readback_uacoff_startup_usbpcap2.pcapng", 0, 0),
];

#[derive(Debug, Clone)]
struct UsbPacket {
    data: Vec<u8>,
    captured_len: usize,
    original_len: usize,
}

#[derive(Debug, Clone, Copy)]
struct UsbHeader {
    header_len: usize,
    /// USBPcap stores the device address as a little-endian u16 at bytes
    /// 19..=20.  The high byte is normally zero, but retaining the full
    /// field avoids treating byte 20 as an endpoint on 27-byte records.
    device: u16,
    endpoint: u8,
    transfer_type: u8,
    data_len: usize,
    irp_id: [u8; 8],
    status: u32,
}

#[derive(Debug, Clone)]
struct ControlTransfer {
    header: UsbHeader,
    payload: Vec<u8>,
    captured_len: usize,
    original_len: usize,
}

#[derive(Debug, Clone)]
struct SetReport {
    header: UsbHeader,
    setup: [u8; 8],
    report: Vec<u8>,
    captured_len: usize,
    original_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GetReportReadback {
    requested_len: usize,
    report: Vec<u8>,
}

fn capture_path(name: &str) -> PathBuf {
    let repository_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("captures")
        .join(name);
    if repository_fixture.is_file() {
        repository_fixture
    } else {
        // Keep compatibility with the original workstation layout while the
        // tracked fixtures make a clean clone self-contained.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("captures")
            .join(name)
    }
}

fn profile_evidence_path(name: &str) -> PathBuf {
    capture_path(PROFILE_EVIDENCE_DIR).join(name)
}

fn hardware_evidence_path(name: &str) -> PathBuf {
    capture_path(HARDWARE_EVIDENCE_DIR).join(name)
}

fn read_u32(bytes: &[u8], offset: usize, little_endian: bool) -> u32 {
    let value = [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ];
    if little_endian {
        u32::from_le_bytes(value)
    } else {
        u32::from_be_bytes(value)
    }
}

fn read_capture_bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("read capture {}: {error}", path.display()))
}

// Keep the evidence test self-contained: profile fingerprints are part of the
// fixture contract, and pulling a hashing crate into the application for a
// test-only assertion would widen the production dependency surface.
fn sha256_hex(bytes: &[u8]) -> String {
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut padded = bytes.to_vec();
    let bit_len = (padded.len() as u64) * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (word, encoded) in schedule[..16].iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let mut working = state;
        for (index, constant) in ROUND_CONSTANTS.iter().enumerate() {
            let choice = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let sigma1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let sigma0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let temp1 = working[7]
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(*constant)
                .wrapping_add(schedule[index]);
            let temp2 = sigma0.wrapping_add(majority);
            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3].wrapping_add(temp1);
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp1.wrapping_add(temp2);
        }
        for (state_word, working_word) in state.iter_mut().zip(working) {
            *state_word = state_word.wrapping_add(working_word);
        }
    }

    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
}

fn parse_pcapng(bytes: &[u8], path: &Path) -> Vec<UsbPacket> {
    assert!(
        bytes.len() >= 12,
        "pcapng file is truncated: {}",
        path.display()
    );

    let mut offset = 0usize;
    let mut little_endian = true;
    let mut packets = Vec::new();
    let mut interface_count = 0usize;
    while offset + 12 <= bytes.len() {
        let block_type = read_u32(&bytes, offset, little_endian);
        if block_type == 0x0A0D0D0A {
            let byte_order = u32::from_le_bytes([
                bytes[offset + 8],
                bytes[offset + 9],
                bytes[offset + 10],
                bytes[offset + 11],
            ]);
            little_endian = match byte_order {
                0x1A2B3C4D => true,
                0x4D3C2B1A => false,
                other => panic!("invalid pcapng byte-order magic: {other:#x}"),
            };
        }

        let block_len = read_u32(&bytes, offset + 4, little_endian) as usize;
        assert!(block_len >= 12, "invalid pcapng block length {block_len}");
        let block_end = offset
            .checked_add(block_len)
            .expect("pcapng block offset overflow");
        assert!(
            block_end <= bytes.len(),
            "pcapng block extends past file end"
        );
        assert_eq!(
            read_u32(&bytes, block_end - 4, little_endian) as usize,
            block_len,
            "pcapng block trailing length mismatch"
        );

        // Enhanced Packet Block.  Its body starts after the 8-byte block
        // header; packet bytes start after the five fixed u32 fields.
        if block_type == 0x00000001 {
            assert!(block_len >= 20, "truncated interface description block");
            let link_type = read_u16(&bytes, offset + 8, little_endian);
            assert_eq!(link_type, 249, "capture is not a USBPcap link type");
            interface_count += 1;
        }
        if block_type == 0x00000006 {
            let body = offset + 8;
            assert!(block_len >= 8 + 20 + 4, "truncated enhanced packet block");
            let captured_len = read_u32(&bytes, body + 12, little_endian) as usize;
            let original_len = read_u32(&bytes, body + 16, little_endian) as usize;
            assert!(
                original_len >= captured_len,
                "original packet length is smaller than the captured length"
            );
            let packet_start = body + 20;
            let packet_end = packet_start
                .checked_add(captured_len)
                .expect("pcapng captured length overflow");
            assert!(
                packet_end <= block_end - 4,
                "captured packet extends past block"
            );
            let padded_len = captured_len
                .checked_add(3)
                .expect("pcapng captured length padding overflow")
                & !3;
            assert!(
                packet_start + padded_len <= block_end - 4,
                "pcapng packet padding extends past block"
            );
            packets.push(UsbPacket {
                data: bytes[packet_start..packet_end].to_vec(),
                captured_len,
                original_len,
            });
        }

        offset = block_end;
    }
    assert_eq!(offset, bytes.len(), "pcapng trailing bytes are not a block");
    assert!(
        interface_count > 0,
        "pcapng has no interface description block"
    );
    packets
}

fn pcapng_packets(path: &Path) -> Vec<UsbPacket> {
    let bytes = read_capture_bytes(path);
    assert_eq!(&bytes[..4], [0x0A, 0x0D, 0x0D, 0x0A], "not a pcapng file");
    parse_pcapng(&bytes, path)
}

fn parse_pcap(bytes: &[u8], path: &Path) -> Vec<UsbPacket> {
    assert!(
        bytes.len() >= 24,
        "pcap file is truncated: {}",
        path.display()
    );
    let magic = &bytes[..4];
    let little_endian = match magic {
        [0xD4, 0xC3, 0xB2, 0xA1] => true,
        [0xA1, 0xB2, 0xC3, 0xD4] => false,
        other => panic!("unsupported classic pcap magic {other:02X?}"),
    };
    let version_major = read_u16(&bytes, 4, little_endian);
    let version_minor = read_u16(&bytes, 6, little_endian);
    assert_eq!((version_major, version_minor), (2, 4));
    assert_eq!(
        read_u32(&bytes, 20, little_endian),
        249,
        "unexpected link type"
    );

    let mut offset = 24usize;
    let mut packets = Vec::new();
    while offset + 16 <= bytes.len() {
        let captured_len = read_u32(&bytes, offset + 8, little_endian) as usize;
        let original_len = read_u32(&bytes, offset + 12, little_endian) as usize;
        assert!(
            original_len >= captured_len,
            "original packet length is smaller than the captured length"
        );
        let record_start = offset + 16;
        let record_end = record_start
            .checked_add(captured_len)
            .expect("classic pcap captured length overflow");
        assert!(
            record_end <= bytes.len(),
            "classic pcap packet extends past file end"
        );
        packets.push(UsbPacket {
            data: bytes[record_start..record_end].to_vec(),
            captured_len,
            original_len,
        });
        offset = record_end;
    }
    assert_eq!(
        offset,
        bytes.len(),
        "classic pcap trailing bytes are not a record"
    );
    packets
}

fn read_u16(bytes: &[u8], offset: usize, little_endian: bool) -> u16 {
    let value = [bytes[offset], bytes[offset + 1]];
    if little_endian {
        u16::from_le_bytes(value)
    } else {
        u16::from_be_bytes(value)
    }
}

fn capture_packets(path: &Path) -> Vec<UsbPacket> {
    let bytes = read_capture_bytes(path);
    assert!(
        bytes.len() >= 4,
        "capture file is truncated: {}",
        path.display()
    );
    if bytes[..4] == [0x0A, 0x0D, 0x0D, 0x0A] {
        parse_pcapng(&bytes, path)
    } else {
        parse_pcap(&bytes, path)
    }
}

fn assert_lossless_hardware_capture(
    fixture: &str,
    expected_bytes: usize,
    expected_records: usize,
    expected_sha256: &str,
) -> Vec<UsbPacket> {
    let path = capture_path(fixture);
    let bytes = read_capture_bytes(&path);
    assert_eq!(
        bytes.len(),
        expected_bytes,
        "hardware evidence file length changed: {}",
        path.display()
    );
    assert_eq!(
        sha256_hex(&bytes),
        expected_sha256,
        "hardware evidence SHA-256 changed: {}",
        path.display()
    );
    let packets = parse_pcap(&bytes, &path);
    assert_eq!(
        packets.len(),
        expected_records,
        "hardware evidence record count changed: {}",
        path.display()
    );
    assert_exact_capture_lengths(&packets);
    packets
}

fn usb_header(packet: &[u8]) -> Option<UsbHeader> {
    if packet.len() < 27 {
        return None;
    }
    let header_len = u16::from_le_bytes([packet[0], packet[1]]) as usize;
    // USBPcap uses a 27-byte header for some interrupt records and a
    // 28-byte header for the control records used below.
    if header_len < 27 || header_len > packet.len() {
        return None;
    }
    let irp_id = packet[2..10].try_into().ok()?;
    Some(UsbHeader {
        header_len,
        device: u16::from_le_bytes([packet[19], packet[20]]),
        endpoint: packet[21],
        transfer_type: packet[22],
        data_len: u32::from_le_bytes([packet[23], packet[24], packet[25], packet[26]]) as usize,
        irp_id,
        status: u32::from_le_bytes([packet[10], packet[11], packet[12], packet[13]]),
    })
}

fn target_address(packets: &[UsbPacket]) -> u16 {
    packets
        .iter()
        .find_map(|packet| {
            packet
                .data
                .windows(TARGET_VID_PID_DESCRIPTOR.len())
                .position(|window| window == TARGET_VID_PID_DESCRIPTOR)
                .and_then(|_| usb_header(&packet.data).map(|header| header.device))
        })
        .expect("target VID/PID descriptor not found")
}

fn target_control_records(packets: &[UsbPacket], target: u16) -> Vec<ControlTransfer> {
    packets
        .iter()
        .filter_map(|packet| {
            let header = usb_header(&packet.data)?;
            if header.device != target || header.transfer_type != 2 {
                return None;
            }
            let payload_end = header
                .header_len
                .checked_add(header.data_len)
                .expect("target control data length overflow");
            assert!(
                payload_end <= packet.data.len(),
                "target control payload extends past the captured packet"
            );
            assert_eq!(
                payload_end,
                packet.data.len(),
                "target control captured length must include exactly its USBPcap data"
            );
            Some(ControlTransfer {
                header,
                payload: packet.data[header.header_len..payload_end].to_vec(),
                captured_len: packet.captured_len,
                original_len: packet.original_len,
            })
        })
        .collect()
}

fn target_control_transfers(packets: &[UsbPacket], target: u16) -> Vec<ControlTransfer> {
    target_control_records(packets, target)
        .into_iter()
        .filter(|control| control.header.endpoint == 0)
        .collect()
}

fn decode_set_report(control: ControlTransfer) -> Option<SetReport> {
    let payload = &control.payload;
    if payload.first() != Some(&0x21) {
        return None;
    }
    assert!(
        payload.len() >= 2,
        "truncated HID class request with bmRequestType=0x21"
    );
    // bmRequestType=0x21, bRequest=0x09 is HID class SET_REPORT.  Any such
    // request is evidence-bearing and must fail loudly if its setup does not
    // match the observed feature-report route.
    if payload[1] != 0x09 {
        return None;
    }
    assert!(payload.len() >= 8, "truncated HID SET_REPORT setup packet");
    let setup: [u8; 8] = payload[..8].try_into().expect("SET_REPORT setup");
    assert_eq!(setup[3], 0x03, "SET_REPORT must select Feature report type");
    assert_eq!(
        u16::from_le_bytes([setup[4], setup[5]]),
        0x0002,
        "SET_REPORT must target the verified configuration interface"
    );
    let report_len = u16::from_le_bytes([setup[6], setup[7]]) as usize;
    assert!(report_len > 0, "SET_REPORT must carry a report");
    assert_eq!(
        report_len + 8,
        payload.len(),
        "SET_REPORT wLength must equal the captured report bytes"
    );
    let report = payload[8..].to_vec();
    assert_eq!(setup[2], report[0], "SET_REPORT ID must match the report");
    assert_eq!(
        control.header.data_len,
        payload.len(),
        "USBPcap data length must equal setup plus report"
    );
    assert_eq!(
        control.captured_len,
        control.header.header_len + control.header.data_len,
        "captured packet length must equal USBPcap header plus data"
    );
    Some(SetReport {
        header: control.header,
        setup,
        report,
        captured_len: control.captured_len,
        original_len: control.original_len,
    })
}

fn set_report_records(packets: &[UsbPacket], target: u16) -> Vec<SetReport> {
    target_control_transfers(packets, target)
        .into_iter()
        .filter_map(decode_set_report)
        .collect()
}

fn set_report_payloads(packets: &[UsbPacket], target: u16) -> Vec<Vec<u8>> {
    set_report_records(packets, target)
        .into_iter()
        .map(|record| record.report)
        .collect()
}

fn decode_get_report_request(control: &ControlTransfer) -> Option<(u8, usize)> {
    let payload = &control.payload;
    if payload.first() != Some(&0xA1) {
        return None;
    }
    assert!(
        payload.len() >= 2,
        "truncated HID class request with bmRequestType=0xA1"
    );
    // bmRequestType=0xA1, bRequest=0x01 is HID class GET_REPORT.  Other
    // class requests are not readback evidence and are left to the enclosing
    // capture-specific checks.
    if payload[1] != 0x01 {
        return None;
    }
    assert_eq!(
        payload.len(),
        8,
        "GET_REPORT request must contain setup only"
    );
    assert_eq!(
        payload[3], 0x03,
        "GET_REPORT must select Feature report type"
    );
    assert_eq!(
        u16::from_le_bytes([payload[4], payload[5]]),
        0x0002,
        "GET_REPORT must target the verified configuration interface"
    );
    let expected_len = u16::from_le_bytes([payload[6], payload[7]]) as usize;
    assert!(
        matches!(expected_len, 16 | 64 | 1024),
        "GET_REPORT wLength must select a supported report route"
    );
    assert_eq!(
        control.header.endpoint, 0x80,
        "GET_REPORT request must use the observed device-to-host control route"
    );
    assert_eq!(
        control.header.data_len,
        payload.len(),
        "GET_REPORT USBPcap data length must equal its setup"
    );
    assert_eq!(
        control.captured_len,
        control.header.header_len + control.header.data_len,
        "GET_REPORT captured length must equal USBPcap header plus setup"
    );
    Some((payload[2], expected_len))
}

fn get_report_requests(packets: &[UsbPacket], target: u16) -> usize {
    target_control_records(packets, target)
        .into_iter()
        .filter(|control| decode_get_report_request(control).is_some())
        .count()
}

/// Pair every target GET_REPORT setup with the later target response carrying
/// the requested feature-report length and ID.  A lone request or response is
/// never counted as readback evidence; both sides of the same-IRP pair must
/// be present and structurally valid.
fn assert_get_report_readback_pairs(packets: &[UsbPacket], target: u16) -> Vec<GetReportReadback> {
    let mut pending: HashMap<[u8; 8], Vec<(u8, usize)>> = HashMap::new();
    let mut request_count = 0usize;
    let mut readbacks = Vec::new();

    for control in target_control_records(packets, target) {
        if let Some((report_id, expected_len)) = decode_get_report_request(&control) {
            assert_eq!(
                control.header.status, 0,
                "GET_REPORT request must have zero status"
            );
            pending
                .entry(control.header.irp_id)
                .or_default()
                .push((report_id, expected_len));
            request_count += 1;
            continue;
        }

        let Some(requests) = pending.get_mut(&control.header.irp_id) else {
            continue;
        };
        // A matching response is the first later record for the IRP with a
        // non-empty payload.  A zero-length completion, if present, is not a
        // readback payload and is checked by the SET/interrupt pairing paths.
        if control.header.data_len == 0 {
            continue;
        }
        let (report_id, expected_len) = requests.remove(0);
        assert_eq!(
            control.header.status, 0,
            "GET_REPORT response must have zero status"
        );
        assert_eq!(
            control.header.endpoint, 0x80,
            "GET_REPORT response must use the observed device-to-host control route"
        );
        assert_eq!(
            control.payload.len(),
            control.header.data_len,
            "GET_REPORT response payload length must match USBPcap data length"
        );
        assert_eq!(
            control.payload.len() <= expected_len,
            true,
            "GET_REPORT response cannot exceed the requested route length"
        );
        assert_eq!(
            control.captured_len,
            control.header.header_len + control.payload.len(),
            "GET_REPORT response captured length must be exact"
        );
        assert_eq!(
            control.payload.first().copied(),
            Some(report_id),
            "GET_REPORT response ID must match its request"
        );
        readbacks.push(GetReportReadback {
            requested_len: expected_len,
            report: control.payload,
        });
        if requests.is_empty() {
            pending.remove(&control.header.irp_id);
        }
    }

    assert!(
        pending.values().all(Vec::is_empty),
        "every target GET_REPORT must have a later matching readback response"
    );
    assert_eq!(
        readbacks.len(),
        request_count,
        "every target GET_REPORT must be paired exactly once"
    );
    readbacks
}

fn assert_set_report_completions(packets: &[UsbPacket], target: u16) {
    let mut pending = HashMap::new();
    let mut set_count = 0usize;
    let mut matched_count = 0usize;

    for control in target_control_transfers(packets, target) {
        if let Some(set) = decode_set_report(control.clone()) {
            assert_eq!(
                set.header.status, 0,
                "SET_REPORT request must have zero status"
            );
            *pending.entry(set.header.irp_id).or_insert(0usize) += 1;
            set_count += 1;
        } else if control.header.data_len == 0 {
            // A reconnect can contain failed enumeration completions that do
            // not belong to a SET_REPORT IRP.  They remain observable in the
            // fixture's status histogram; only a completion matched to a
            // captured SET_REPORT is required to be zero-status here.
            if let Some(count) = pending.get_mut(&control.header.irp_id) {
                assert_eq!(
                    control.header.status, 0,
                    "SET_REPORT completion must report zero status"
                );
                assert!(*count > 0, "completion pairing underflow");
                *count -= 1;
                matched_count += 1;
            }
        }
    }

    assert!(
        pending.values().all(|count| *count == 0),
        "every target SET_REPORT must have a later zero-status completion with the same IRP"
    );
    assert_eq!(
        matched_count, set_count,
        "every target SET_REPORT must be paired exactly once"
    );
}

/// Check a contiguous subset of the target SET_REPORT stream.  This is used
/// for the physical-run Apply burst so that all 156 request/completion pairs
/// are checked independently of the resident traffic before and after it.
fn assert_set_report_completions_for_range(
    packets: &[UsbPacket],
    target: u16,
    first_set: usize,
    expected_count: usize,
) {
    let controls = target_control_transfers(packets, target);
    let indexed_sets: Vec<(usize, SetReport)> = controls
        .iter()
        .enumerate()
        .filter_map(|(index, control)| decode_set_report(control.clone()).map(|set| (index, set)))
        .collect();
    let selected = indexed_sets
        .get(first_set..first_set + expected_count)
        .unwrap_or_else(|| {
            panic!(
                "SET_REPORT range {first_set}..{} is missing",
                first_set + expected_count
            )
        });
    assert_eq!(selected.len(), expected_count);

    let mut search_from = selected[0].0 + 1;
    let mut matched_count = 0usize;
    for (set_index, set) in selected {
        assert_eq!(
            set.header.status, 0,
            "Apply SET_REPORT request must be zero-status"
        );
        assert!(*set_index >= search_from - 1);
        let completion_offset = controls[search_from..]
            .iter()
            .position(|control| {
                control.header.irp_id == set.header.irp_id && control.header.data_len == 0
            })
            .unwrap_or_else(|| panic!("Apply SET_REPORT at control {set_index} has no completion"));
        let completion_index = search_from + completion_offset;
        assert_eq!(
            controls[completion_index].header.status, 0,
            "Apply SET_REPORT completion must be zero-status"
        );
        search_from = completion_index + 1;
        matched_count += 1;
    }
    assert_eq!(matched_count, expected_count);
}

fn assert_no_get_report_for_set_range(
    packets: &[UsbPacket],
    target: u16,
    first_set: usize,
    expected_count: usize,
) {
    let controls = target_control_records(packets, target);
    let set_indices: Vec<usize> = controls
        .iter()
        .enumerate()
        .filter_map(|(index, control)| {
            (control.header.endpoint == 0)
                .then(|| decode_set_report(control.clone()).map(|_| index))
                .flatten()
        })
        .collect();
    let first = *set_indices
        .get(first_set)
        .unwrap_or_else(|| panic!("SET_REPORT start {first_set} is missing"));
    let last = *set_indices
        .get(first_set + expected_count - 1)
        .unwrap_or_else(|| {
            panic!(
                "SET_REPORT end {} is missing",
                first_set + expected_count - 1
            )
        });
    let get_count = controls[first..=last]
        .iter()
        .filter(|control| decode_get_report_request(control).is_some())
        .count();
    assert_eq!(
        get_count, 0,
        "the official Apply window must contain no GET_REPORT"
    );
}

fn assert_interrupt_completions(packets: &[UsbPacket], target: u16, expected: usize) {
    let mut pending = HashMap::new();
    let mut data_count = 0usize;
    let mut completion_count = 0usize;
    let mut unmatched_completion_count = 0usize;
    for packet in packets {
        let Some(header) = usb_header(&packet.data) else {
            continue;
        };
        if header.device != target || header.endpoint != 0x81 || header.transfer_type != 1 {
            continue;
        }
        assert_eq!(
            packet.captured_len,
            header.header_len + header.data_len,
            "captured interrupt packet length must equal USBPcap header plus data"
        );
        match header.data_len {
            8 => {
                *pending.entry(header.irp_id).or_insert(0usize) += 1;
                data_count += 1;
            }
            0 => {
                assert_eq!(
                    header.status, 0,
                    "interrupt completion must report zero status"
                );
                if let Some(count) = pending.get_mut(&header.irp_id) {
                    assert!(*count > 0, "interrupt completion pairing underflow");
                    *count -= 1;
                    completion_count += 1;
                } else {
                    unmatched_completion_count += 1;
                }
            }
            other => panic!("unexpected target interrupt data length {other}"),
        }
    }
    assert_eq!(data_count, expected);
    assert_eq!(completion_count, expected);
    assert_eq!(unmatched_completion_count, 0);
    assert!(pending.values().all(|count| *count == 0));
}

fn assert_exact_capture_lengths(packets: &[UsbPacket]) {
    assert!(
        !packets.is_empty(),
        "capture must contain at least one packet"
    );
    for packet in packets {
        assert_eq!(
            packet.captured_len,
            packet.data.len(),
            "parser must retain exactly the captured packet bytes"
        );
        assert!(
            packet.original_len >= packet.captured_len,
            "original packet length cannot be smaller than captured length"
        );
        assert_eq!(
            packet.original_len, packet.captured_len,
            "controlled evidence must not silently include a truncated packet"
        );
    }
}

/// Resolve the target from the descriptor rather than from a hard-coded USB
/// address.  A physical reconnect is allowed to receive a new bus address;
/// only the descriptor-associated address is evidence-bearing.
fn assert_target_capture_attribution(packets: &[UsbPacket]) -> u16 {
    let target = target_address(packets);

    let descriptor_devices: Vec<u16> = packets
        .iter()
        .filter(|packet| {
            packet
                .data
                .windows(TARGET_VID_PID_DESCRIPTOR.len())
                .any(|window| window == TARGET_VID_PID_DESCRIPTOR)
        })
        .filter_map(|packet| usb_header(&packet.data).map(|header| header.device))
        .collect();
    assert!(
        !descriptor_devices.is_empty(),
        "target descriptor must be observed"
    );
    assert!(
        descriptor_devices.iter().all(|device| *device == target),
        "every VID/PID descriptor observation must map to one target address"
    );

    let target_packets = packets
        .iter()
        .filter_map(|packet| usb_header(&packet.data).map(|header| (packet, header)))
        .filter(|(_, header)| header.device == target)
        .collect::<Vec<_>>();
    assert!(
        !target_packets.is_empty(),
        "descriptor-identified target must have at least one USBPcap packet"
    );
    for (index, (packet, header)) in target_packets.iter().enumerate() {
        assert_eq!(
            packet.captured_len,
            header.header_len + header.data_len,
            "target packet {index} length does not match its USBPcap header"
        );
    }
    target
}

fn assert_target_attribution(packets: &[UsbPacket]) -> u16 {
    let target = assert_target_capture_attribution(packets);
    assert_eq!(target, 2, "the evidence target must remain USB address 2");
    target
}

fn packets_for_target(packets: &[UsbPacket], target: u16) -> Vec<UsbPacket> {
    packets
        .iter()
        .filter_map(|packet| {
            usb_header(&packet.data)
                .filter(|header| header.device == target)
                .map(|_| packet.clone())
        })
        .collect()
}

fn assert_descriptor_attribution(packets: &[UsbPacket], target: u16, expected_count: usize) {
    let descriptor_devices: Vec<u16> = packets
        .iter()
        .filter(|packet| {
            packet
                .data
                .windows(TARGET_VID_PID_DESCRIPTOR.len())
                .any(|window| window == TARGET_VID_PID_DESCRIPTOR)
        })
        .filter_map(|packet| usb_header(&packet.data).map(|header| header.device))
        .collect();
    assert_eq!(
        descriptor_devices.len(),
        expected_count,
        "target descriptor observation count changed"
    );
    assert!(
        descriptor_devices.iter().all(|device| *device == target),
        "every VID/PID descriptor must remain attributed to the target address"
    );
}

fn assert_target_control_status_histogram(
    packets: &[UsbPacket],
    target: u16,
    expected: &[(u32, usize)],
) {
    let actual = target_control_records(packets, target).into_iter().fold(
        HashMap::new(),
        |mut counts, control| {
            *counts.entry(control.header.status).or_insert(0usize) += 1;
            counts
        },
    );
    let expected: HashMap<u32, usize> = expected.iter().copied().collect();
    assert_eq!(actual, expected, "target control status histogram changed");
}

fn assert_packet_length_histogram(packets: &[UsbPacket], expected: &[(usize, usize)]) {
    let mut actual = HashMap::new();
    for packet in packets {
        *actual.entry(packet.captured_len).or_insert(0usize) += 1;
    }
    let expected: HashMap<usize, usize> = expected.iter().copied().collect();
    assert_eq!(
        actual, expected,
        "capture packet length distribution changed"
    );
}

/// Check the complete USB enumeration prefix emitted by the resident and
/// reconnect captures.  These six records are three request/response pairs:
/// device descriptor, configuration descriptor, and SET_CONFIGURATION with a
/// zero-length completion.  Their exact payloads also pin target attribution
/// to the observed VID/PID rather than to an unrelated bus device.
fn assert_enumeration_control_records_with_irp_policy(
    packets: &[UsbPacket],
    target: u16,
    require_zero_irp: bool,
) {
    let controls = target_control_records(packets, target);
    assert!(
        controls.len() >= ENUMERATION_CONTROL_PAYLOADS.len(),
        "enumeration/reconnect control-record prefix is missing"
    );

    for (index, (control, expected_payload)) in controls[..ENUMERATION_CONTROL_PAYLOADS.len()]
        .iter()
        .zip(ENUMERATION_CONTROL_PAYLOADS)
        .enumerate()
    {
        let expected = hex_report(expected_payload);
        assert_eq!(
            control.header.header_len, 28,
            "enumeration control header length changed at record {index}"
        );
        assert_eq!(
            control.header.transfer_type, 2,
            "enumeration transfer type changed at record {index}"
        );
        assert_eq!(
            control.header.status, 0,
            "enumeration control status changed at record {index}"
        );
        if require_zero_irp {
            assert_eq!(
                control.header.irp_id, [0u8; 8],
                "enumeration IRP identity changed at record {index}"
            );
        }
        let expected_endpoint = if index < 4 { 0x80 } else { 0x00 };
        assert_eq!(
            control.header.endpoint, expected_endpoint,
            "enumeration endpoint changed at record {index}"
        );
        assert_eq!(
            control.payload, expected,
            "enumeration control payload changed at record {index}"
        );
        assert_eq!(
            control.header.data_len,
            control.payload.len(),
            "enumeration USBPcap data length changed at record {index}"
        );
        assert_eq!(
            control.captured_len,
            control.header.header_len + control.payload.len(),
            "enumeration captured length changed at record {index}"
        );
        assert_eq!(
            control.original_len, control.captured_len,
            "enumeration record is unexpectedly truncated at record {index}"
        );
    }

    for (request, response) in [(0usize, 1usize), (2, 3), (4, 5)] {
        assert_eq!(
            controls[request].header.irp_id, controls[response].header.irp_id,
            "enumeration request/response IRP mismatch for pair {request}/{response}"
        );
        assert_eq!(
            controls[request].header.status, controls[response].header.status,
            "enumeration request/response status mismatch for pair {request}/{response}"
        );
    }
}

fn assert_enumeration_control_records(packets: &[UsbPacket], target: u16) {
    assert_enumeration_control_records_with_irp_policy(packets, target, true);
}

fn assert_hardware_enumeration_control_records(packets: &[UsbPacket], target: u16) {
    let controls = target_control_records(packets, target);
    assert!(
        controls.len() >= HARDWARE_ENUMERATION_CONTROL_PAYLOADS.len(),
        "hardware enumeration control-record prefix is missing"
    );
    for (index, (control, expected_payload)) in controls
        [..HARDWARE_ENUMERATION_CONTROL_PAYLOADS.len()]
        .iter()
        .zip(HARDWARE_ENUMERATION_CONTROL_PAYLOADS)
        .enumerate()
    {
        let expected = hex_report(expected_payload);
        assert_eq!(control.header.header_len, 28);
        assert_eq!(control.header.transfer_type, 2);
        assert_eq!(control.header.status, 0);
        assert_eq!(
            control.header.endpoint,
            if index < 6 { 0x80 } else { 0x00 },
            "hardware enumeration endpoint changed at record {index}"
        );
        assert_eq!(control.payload, expected);
        assert_eq!(control.header.data_len, control.payload.len());
        assert_eq!(
            control.captured_len,
            control.header.header_len + control.payload.len()
        );
        assert_eq!(control.original_len, control.captured_len);
    }
    for (request, response) in [(0usize, 1usize), (2, 3), (4, 5), (6, 7)] {
        assert_eq!(
            controls[request].header.irp_id, controls[response].header.irp_id,
            "hardware enumeration request/response IRP mismatch for pair {request}/{response}"
        );
        assert_eq!(
            controls[request].header.status, controls[response].header.status,
            "hardware enumeration request/response status mismatch for pair {request}/{response}"
        );
    }
}

fn assert_reconnect_capture_shape(packets: &[UsbPacket], target: u16) {
    assert_eq!(
        packets.len(),
        RECONNECT_EVIDENCE_RECORDS,
        "reconnect capture record count changed"
    );
    assert_packet_length_histogram(
        packets,
        &[(27, 853), (28, 1), (35, 853), (36, 3), (46, 1), (112, 1)],
    );

    // This fixture is a --devices 2 capture.  Every packet must still carry a
    // valid USBPcap header for the descriptor-identified target; silently
    // skipping malformed or foreign packets would make attribution unsafe.
    for (index, packet) in packets.iter().enumerate() {
        let header = usb_header(&packet.data)
            .unwrap_or_else(|| panic!("reconnect packet {index} has no valid USBPcap header"));
        assert_eq!(
            header.device, target,
            "reconnect packet {index} is attributed to another USB address"
        );
        assert_eq!(
            packet.captured_len,
            header.header_len + header.data_len,
            "reconnect packet {index} length does not match its USBPcap header"
        );
    }

    assert_enumeration_control_records(packets, target);
    assert_interrupt_completions(packets, target, 853);
    assert_eq!(
        set_report_records(packets, target).len(),
        0,
        "failed reconnect evidence must contain no target SET_REPORT"
    );
    assert!(
        assert_get_report_readback_pairs(packets, target).is_empty(),
        "failed reconnect evidence must contain no target GET_REPORT/readback"
    );
    assert_set_report_completions(packets, target);
}

fn assert_profile_hash(path: &Path, expected_hash: &str) {
    let profile = fs::read(path)
        .unwrap_or_else(|error| panic!("read profile evidence {}: {error}", path.display()));
    assert_eq!(
        profile.len(),
        PROFILE_BYTES,
        "profile evidence {} length changed",
        path.display()
    );
    assert_eq!(
        sha256_hex(&profile),
        expected_hash,
        "profile evidence {} SHA-256 changed",
        path.display()
    );
}

/// Require one exact PollingRate readback marker.  A future reconnect A/B
/// capture must provide this marker on both sides; a SET_REPORT write or a
/// lone GET_REPORT setup is not accepted as readback evidence.
fn assert_exact_polling_rate_readback<'a>(
    readbacks: &'a [GetReportReadback],
    expected_report: &str,
) -> &'a GetReportReadback {
    let expected = hex_report(expected_report);
    let matches: Vec<&GetReportReadback> = readbacks
        .iter()
        .filter(|readback| readback.report == expected)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "exactly one PollingRate readback marker must be paired"
    );
    assert_eq!(matches[0].requested_len, 16);
    assert_eq!(matches[0].report, expected);
    matches[0]
}

fn assert_restart_readback_shape(readbacks: &[GetReportReadback], expected_polling_readback: &str) {
    assert_eq!(
        readbacks.len(),
        RESTART_GET_REPORTS,
        "post-restart GET_REPORT/readback count changed"
    );

    let requested_lengths: HashMap<usize, usize> =
        readbacks
            .iter()
            .fold(HashMap::new(), |mut counts, readback| {
                *counts.entry(readback.requested_len).or_insert(0usize) += 1;
                counts
            });
    assert_eq!(
        requested_lengths,
        [(16usize, 108usize), (64, 85)].into_iter().collect(),
        "post-restart GET_REPORT route counts changed"
    );

    let response_lengths: HashMap<usize, usize> =
        readbacks
            .iter()
            .fold(HashMap::new(), |mut counts, readback| {
                *counts.entry(readback.report.len()).or_insert(0usize) += 1;
                counts
            });
    assert_eq!(
        response_lengths,
        [
            (9usize, 1usize),
            (11, 1),
            (12, 100),
            (13, 1),
            (15, 5),
            (40, 5),
            (58, 80),
        ]
        .into_iter()
        .collect(),
        "post-restart readback lengths changed"
    );

    assert_exact_polling_rate_readback(readbacks, expected_polling_readback);
}

fn assert_hardware_reconnect_readback_shape(readbacks: &[GetReportReadback]) {
    assert_eq!(
        readbacks.len(),
        HARDWARE_RECONNECT_GET_REPORTS,
        "physical reconnect GET_REPORT/readback count changed"
    );
    let (pre_reconnect, post_reconnect) = readbacks.split_at(HARDWARE_RESTART_GET_REPORTS);
    assert_eq!(pre_reconnect.len(), HARDWARE_RESTART_GET_REPORTS);
    assert_eq!(post_reconnect.len(), HARDWARE_RESTART_GET_REPORTS);

    for (phase, phase_readbacks) in [
        ("pre-reconnect 250 Hz", pre_reconnect),
        ("post-reconnect 250 Hz", post_reconnect),
    ] {
        let requested_lengths: HashMap<usize, usize> =
            phase_readbacks
                .iter()
                .fold(HashMap::new(), |mut counts, readback| {
                    *counts.entry(readback.requested_len).or_insert(0usize) += 1;
                    counts
                });
        assert_eq!(
            requested_lengths,
            [(16usize, 108usize), (64, 85)].into_iter().collect(),
            "{phase} GET_REPORT route counts changed"
        );

        let response_lengths: HashMap<usize, usize> =
            phase_readbacks
                .iter()
                .fold(HashMap::new(), |mut counts, readback| {
                    *counts.entry(readback.report.len()).or_insert(0usize) += 1;
                    counts
                });
        assert_eq!(
            response_lengths,
            [
                (9usize, 1usize),
                (11, 1),
                (12, 100),
                (13, 1),
                (15, 5),
                (40, 5),
                (58, 80),
            ]
            .into_iter()
            .collect(),
            "{phase} readback response lengths changed"
        );
        assert_exact_polling_rate_readback(phase_readbacks, POLLING_RATE_READBACK_250_HZ);
    }

    assert_eq!(
        pre_reconnect, post_reconnect,
        "pre- and post-reconnect 250 Hz readbacks must retain the same paired state"
    );
}

fn assert_hardware_profile_metadata(name: &str, expected_hash: &str) {
    let path = hardware_evidence_path(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read profile metadata {}: {error}", path.display()));
    assert!(
        text.lines().any(|line| line == "profileBytes=21672"),
        "profile metadata {} must retain the 21,672-byte evidence",
        path.display()
    );
    let expected_line = format!("profileSha256={expected_hash}");
    assert!(
        text.lines().any(|line| line == expected_line),
        "profile metadata {} SHA-256 changed",
        path.display()
    );
}

fn assert_hardware_restart_profile_metadata() {
    let path = hardware_evidence_path(HARDWARE_RESTART_STATE);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("read restart profile metadata {}: {error}", path.display())
    });
    let expected_hash = PROFILE_HASH_125_HZ.to_ascii_uppercase();
    assert!(
        text.contains(&format!("\"profileBefore\": \"{expected_hash}\"")),
        "restart evidence profileBefore hash changed"
    );
    assert!(
        text.contains(&format!("\"profileAfter\": \"{expected_hash}\"")),
        "restart evidence profileAfter hash changed"
    );
}

fn assert_hardware_apply_burst(packets: &[UsbPacket], target: u16) {
    let reports = set_report_payloads(packets, target);
    let starts: Vec<usize> = reports
        .iter()
        .enumerate()
        .filter_map(|(index, report)| {
            (report == &hex_report("02f50000000000000000000000000000")).then_some(index)
        })
        .collect();
    assert_eq!(
        starts,
        vec![0, 195, HARDWARE_RECONNECT_APPLY_SET_START],
        "physical-run Apply burst boundaries changed"
    );
    let apply_end = HARDWARE_RECONNECT_APPLY_SET_START + HARDWARE_APPLY_SET_REPORTS;
    assert_eq!(reports.len(), apply_end);
    assert_eq!(
        reports[apply_end - 1],
        hex_report("02f50100000000000000000000000000")
    );
    let apply = &reports[HARDWARE_RECONNECT_APPLY_SET_START..apply_end];
    assert_burst_shape(apply);
    assert_eq!(
        apply[13],
        hex_report("02f33200060000000800080002000000"),
        "physical-run 125 Hz Apply wire value changed"
    );

    // Compare every report in the physical Apply burst with the independent
    // captured 500 Hz -> 125 Hz Apply fixture.  This is evidence equality,
    // not authorization to synthesize or send any frame.
    let reference_path = capture_path("polling_rate_500_to_125_usbpcap2.pcapng");
    let reference_packets = pcapng_packets(&reference_path);
    let reference_target = target_address(&reference_packets);
    let reference_reports = set_report_payloads(&reference_packets, reference_target);
    assert_eq!(apply, reference_reports.as_slice());

    assert_set_report_completions_for_range(
        packets,
        target,
        HARDWARE_RECONNECT_APPLY_SET_START,
        HARDWARE_APPLY_SET_REPORTS,
    );
    assert_no_get_report_for_set_range(
        packets,
        target,
        HARDWARE_RECONNECT_APPLY_SET_START,
        HARDWARE_APPLY_SET_REPORTS,
    );
}

fn assert_restart_profile_matches_readback(readbacks: &[GetReportReadback]) {
    let profile_path = capture_path(CONTROLLED_RUN_DIR).join(PROFILE_BEFORE);
    let profile = fs::read(&profile_path).unwrap_or_else(|error| {
        panic!(
            "read post-restart profile evidence {}: {error}",
            profile_path.display()
        )
    });
    assert_eq!(
        profile.len(),
        PROFILE_BYTES,
        "post-restart profile length changed"
    );
    assert_eq!(
        sha256_hex(&profile),
        PROFILE_HASH_250_HZ,
        "post-restart profile SHA-256 changed"
    );
    assert!(profile
        .windows(b"PollingRate=4\r\n".len())
        .any(|window| { window == b"PollingRate=4\r\n" }));

    let previous_after = fs::read(profile_evidence_path(PROFILE_AFTER))
        .expect("the post-restart profile must have a checked prior Apply artifact");
    assert_eq!(
        profile, previous_after,
        "post-restart profile must match the previously applied profile"
    );

    let polling = assert_exact_polling_rate_readback(readbacks, POLLING_RATE_READBACK_250_HZ);
    assert_eq!(polling.report[8], 0x04);
    assert_eq!(
        polling.report[8], b'\x04',
        "profile/readback polling byte must remain the observed 250 Hz code"
    );
}

fn assert_ui_apply_profile_is_unchanged() {
    let before_path = capture_path(CONTROLLED_RUN_DIR).join(PROFILE_BEFORE);
    let after_path = capture_path(CONTROLLED_RUN_DIR).join(PROFILE_AFTER);
    let before = fs::read(&before_path).unwrap_or_else(|error| {
        panic!(
            "read UI-run profile-before evidence {}: {error}",
            before_path.display()
        )
    });
    let after = fs::read(&after_path).unwrap_or_else(|error| {
        panic!(
            "read UI-run profile-after evidence {}: {error}",
            after_path.display()
        )
    });
    assert_eq!(
        before.len(),
        PROFILE_BYTES,
        "UI-run profile-before length changed"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "UI-run profile-after length changed"
    );
    assert_eq!(
        before, after,
        "official Apply with unchanged 250 Hz selection must not change the profile"
    );
    assert!(after
        .windows(b"PollingRate=4\r\n".len())
        .any(|window| { window == b"PollingRate=4\r\n" }));
    assert_eq!(
        sha256_hex(&before),
        PROFILE_HASH_250_HZ,
        "UI-run profile-before SHA-256 changed"
    );
    assert_eq!(
        sha256_hex(&after),
        PROFILE_HASH_250_HZ,
        "UI-run profile-after SHA-256 changed"
    );
}

fn assert_profile_apply_delta() {
    let before_path = profile_evidence_path(PROFILE_BEFORE);
    let after_path = profile_evidence_path(PROFILE_AFTER);
    let before = fs::read(&before_path)
        .unwrap_or_else(|error| panic!("read profile evidence {}: {error}", before_path.display()));
    let after = fs::read(&after_path)
        .unwrap_or_else(|error| panic!("read profile evidence {}: {error}", after_path.display()));
    assert_eq!(before.len(), PROFILE_BYTES, "profile-before length changed");
    assert_eq!(after.len(), before.len(), "profile-after length changed");
    assert_eq!(
        sha256_hex(&before),
        PROFILE_HASH_125_HZ,
        "profile-before SHA-256 changed"
    );
    assert_eq!(
        sha256_hex(&after),
        PROFILE_HASH_250_HZ,
        "profile-after SHA-256 changed"
    );
    assert!(before.starts_with(b"[GROUP0]\r\n"));
    assert!(after.starts_with(b"[GROUP0]\r\n"));
    assert!(before
        .windows(b"PollingRate=8\r\n".len())
        .any(|window| { window == b"PollingRate=8\r\n" }));
    assert!(after
        .windows(b"PollingRate=4\r\n".len())
        .any(|window| { window == b"PollingRate=4\r\n" }));

    let deltas: Vec<(usize, u8, u8)> = before
        .iter()
        .zip(&after)
        .enumerate()
        .filter_map(|(offset, (old, new))| (old != new).then_some((offset, *old, *new)))
        .collect();
    assert_eq!(
        deltas,
        vec![(63, 0x38, 0x34)],
        "profile Apply evidence must retain its one-byte polling-rate delta"
    );
}

fn assert_target_report_routes(
    packets: &[UsbPacket],
    target: u16,
    expected_16: usize,
    expected_64: usize,
) {
    let records = set_report_records(packets, target);
    let mut count_16 = 0usize;
    let mut count_64 = 0usize;

    for record in &records {
        assert_eq!(record.header.endpoint, 0);
        assert_eq!(record.header.transfer_type, 2);
        assert_eq!(record.header.header_len, 28);
        assert_eq!(record.original_len, record.captured_len);
        assert_eq!(
            record.captured_len,
            record.header.header_len + record.header.data_len
        );
        match record.report.len() {
            16 => {
                count_16 += 1;
                assert_eq!(
                    record.setup,
                    hex_report("2109020302001000").as_slice(),
                    "16-byte SET_REPORT setup changed"
                );
                assert_eq!(record.captured_len, 52);
                assert_eq!(record.header.data_len, 24);
            }
            64 => {
                count_64 += 1;
                assert_eq!(
                    record.setup,
                    hex_report("2109030302004000").as_slice(),
                    "64-byte SET_REPORT setup changed"
                );
                assert_eq!(record.captured_len, 100);
                assert_eq!(record.header.data_len, 72);
            }
            other => panic!("unexpected target SET_REPORT route length {other}"),
        }
    }

    assert_eq!(
        count_16, expected_16,
        "unexpected 16-byte target report count"
    );
    assert_eq!(
        count_64, expected_64,
        "unexpected 64-byte target report count"
    );

    let mut unrelated_1024 = 0usize;
    for packet in packets {
        let Some(header) = usb_header(&packet.data) else {
            continue;
        };
        if header.data_len != 1024 {
            continue;
        }
        assert_ne!(
            header.device, target,
            "the unverified 1024-byte candidate must not be attributed to the target"
        );
        assert_eq!(header.endpoint, 2, "unrelated block route endpoint changed");
        assert_eq!(
            header.transfer_type, 1,
            "unrelated block route type changed"
        );
        assert_eq!(
            packet.captured_len,
            header.header_len + 1024,
            "1024-byte block packet length changed"
        );
        assert_eq!(packet.original_len, packet.captured_len);
        unrelated_1024 += 1;
    }
    assert!(
        unrelated_1024 > 0,
        "the evidence fixture must retain unrelated 1024-byte traffic for attribution"
    );
}

fn assert_burst_shape(burst: &[Vec<u8>]) {
    assert_eq!(burst.len(), 156);
    assert_eq!(
        burst.iter().filter(|report| report.len() == 16).count(),
        155
    );
    assert_eq!(burst.iter().filter(|report| report.len() == 64).count(), 1);
    assert!(burst
        .iter()
        .all(|report| report.len() == 16 || report.len() == 64));
    assert_eq!(burst[0], hex_report("02f50000000000000000000000000000"));
    assert_eq!(burst[15][..3], [0x03, 0xF3, 0x20]);
    assert_eq!(burst[15][16], 0x01);
    assert_eq!(burst[15][17..], [0u8; 47]);
    // The complete Apply sequence also carries the same F1-family tail in
    // every capture.  Keep these as observed markers only; their commit or
    // reset semantics are intentionally not inferred here.
    assert_eq!(burst[21], hex_report("02f10202000000000000000000000000"));
    assert_eq!(burst[22], hex_report("02f10210000000000000000000000000"));
    assert_eq!(burst[48], hex_report("02f10210000000000000000000000000"));
    assert_eq!(burst[50], hex_report("02f10201000000000000000000000000"));
    assert_eq!(burst[151], hex_report("02f10204000000000000000000000000"));
    assert_eq!(burst[152], hex_report("02f10201000000000000000000000000"));
    assert_eq!(burst[153], hex_report("02f10202000000000000000000000000"));
    assert_eq!(burst[154], hex_report("02f10208000000000000000000000000"));
    assert_eq!(burst[155], hex_report("02f50100000000000000000000000000"));
}

/// Check a capture made from one or more complete Apply operations.  The
/// expected pairs list only the report positions that changed between two
/// adjacent bursts; every other report must remain byte-identical.  This
/// keeps the fixture useful as a regression check without treating a field as
/// write authorization.
fn assert_apply_bursts(
    name: &str,
    expected_bursts: usize,
    expected_pairs: &[(usize, &[(usize, &str, &str)])],
) {
    let path = capture_path(name);
    let packets = pcapng_packets(&path);
    let target = target_address(&packets);
    let reports = set_report_payloads(&packets, target);

    // A UI capture can contain short interstitial SET_REPORT traffic between
    // complete Apply sequences.  The sequence itself is self-delimiting: it
    // starts with 02/F5/00 and the 156th report is 02/F5/01.  Anchor on those
    // markers instead of assuming that unrelated traffic is absent.
    let starts: Vec<usize> = reports
        .iter()
        .enumerate()
        .filter_map(|(index, report)| {
            (report == &hex_report("02f50000000000000000000000000000")).then_some(index)
        })
        .collect();
    assert_eq!(starts.len(), expected_bursts, "{name}");
    let bursts: Vec<&[Vec<u8>]> = starts
        .iter()
        .map(|&start| {
            assert!(
                start + 156 <= reports.len(),
                "{name}: truncated apply burst"
            );
            assert_eq!(
                reports[start + 155],
                hex_report("02f50100000000000000000000000000"),
                "{name}: apply burst terminator is missing"
            );
            &reports[start..start + 156]
        })
        .collect();
    for burst in &bursts {
        assert_burst_shape(burst);
    }

    for (pair_index, changes) in expected_pairs {
        let before = bursts[*pair_index];
        let after = bursts[*pair_index + 1];
        for report_index in 0..156 {
            let expected = changes.iter().find(|(index, _, _)| *index == report_index);
            match expected {
                Some((_, before_hex, after_hex)) => {
                    assert_eq!(before[report_index], hex_report(before_hex));
                    assert_eq!(after[report_index], hex_report(after_hex));
                }
                None => assert_eq!(
                    before[report_index],
                    after[report_index],
                    "{name}: unexpected difference at pair {}, report {}",
                    pair_index,
                    report_index + 1
                ),
            }
        }
    }

    // The target has no captured 1024-byte transfer.  The same files do
    // contain unrelated large traffic from another USB address; keep that
    // traffic out of the target attribution.
    assert!(packets.iter().all(|packet| {
        usb_header(&packet.data)
            .map(|header| !(header.device == target && header.data_len == 1024))
            .unwrap_or(true)
    }));
    assert!(packets.iter().any(|packet| {
        usb_header(&packet.data)
            .map(|header| header.device != target && header.data_len == 1024)
            .unwrap_or(false)
    }));
}

fn assert_paired_captures_with_delta(
    first_name: &str,
    second_name: &str,
    before_delta: &str,
    after_delta: &str,
) {
    let first_path = capture_path(first_name);
    let second_path = capture_path(second_name);
    let first_packets = pcapng_packets(&first_path);
    let second_packets = pcapng_packets(&second_path);

    let first_target = target_address(&first_packets);
    let second_target = target_address(&second_packets);
    assert_eq!(
        first_target, second_target,
        "target address changed between fixtures"
    );

    let first = set_report_payloads(&first_packets, first_target);
    let second = set_report_payloads(&second_packets, second_target);
    assert_burst_shape(&first);
    assert_burst_shape(&second);
    assert_eq!(
        first.len(),
        second.len(),
        "paired captures must have equal report counts"
    );
    assert_eq!(second[0], first[0]);
    assert_eq!(second[15], first[15]);
    assert_eq!(second[155], first[155]);

    for (index, (a, b)) in first.iter().zip(&second).enumerate() {
        if index == 13 {
            assert_eq!(a, &hex_report(before_delta));
            assert_eq!(b, &hex_report(after_delta));
            assert_eq!(
                a.iter()
                    .zip(b)
                    .filter(|(before, after)| before != after)
                    .count(),
                1,
                "the paired readback delta must change exactly one byte"
            );
            continue;
        }
        assert_eq!(a, b, "paired captures differ at report {}", index + 1);
    }

    // The target's captured SET_REPORTs contain no 1024-byte block.  Any
    // unrelated 1024-byte traffic in the file must not be attributed to it.
    assert!(first_packets.iter().all(|packet| {
        usb_header(&packet.data)
            .map(|header| !(header.device == first_target && header.data_len == 1024))
            .unwrap_or(true)
    }));
    assert!(first_packets.iter().any(|packet| {
        usb_header(&packet.data)
            .map(|header| header.device != first_target && header.data_len == 1024)
            .unwrap_or(false)
    }));
    assert!(second_packets.iter().all(|packet| {
        usb_header(&packet.data)
            .map(|header| !(header.device == second_target && header.data_len == 1024))
            .unwrap_or(true)
    }));
}

fn assert_paired_captures(first_name: &str, second_name: &str) {
    assert_paired_captures_with_delta(
        first_name,
        second_name,
        "02f33200060000000400080002000000",
        "02f33200060000000100080002000000",
    );
}

fn assert_reconnect_ab_snapshot(
    packets: &[UsbPacket],
    profile_path: &Path,
    expected_profile_hash: &str,
    expected_polling_readback: &str,
) -> u16 {
    assert_exact_capture_lengths(packets);
    let target = assert_target_capture_attribution(packets);
    let readbacks = assert_get_report_readback_pairs(packets, target);
    assert_exact_polling_rate_readback(&readbacks, expected_polling_readback);
    assert_profile_hash(profile_path, expected_profile_hash);
    target
}

/// Build a small in-memory USBPcap stream for the reconnect A/B contract.  It
/// intentionally uses different addresses for A and B: a physical reconnect
/// may keep or change the bus address, so the descriptor must be the source of
/// attribution.  This does not stand in for physical evidence; it only keeps
/// the fail-loud pairing and value checks exercised until that fixture exists.
fn synthetic_usb_packet(
    device: u16,
    endpoint: u8,
    transfer_type: u8,
    irp_id: [u8; 8],
    payload: &[u8],
) -> UsbPacket {
    let header_len = 28usize;
    let mut data = vec![0u8; header_len];
    data[..2].copy_from_slice(&(header_len as u16).to_le_bytes());
    data[2..10].copy_from_slice(&irp_id);
    data[19..21].copy_from_slice(&device.to_le_bytes());
    data[21] = endpoint;
    data[22] = transfer_type;
    data[23..27].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(payload);
    UsbPacket {
        captured_len: data.len(),
        original_len: data.len(),
        data,
    }
}

fn synthetic_reconnect_readback_capture(device: u16, polling_readback: &str) -> Vec<UsbPacket> {
    let irp_id = [0xA5, 0x5A, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    let descriptor = hex_report("1201000200000008d90455fc100200020001");
    let request = hex_report("a101020302001000");
    let response = hex_report(polling_readback);
    vec![
        synthetic_usb_packet(device, 0x80, 2, [0u8; 8], &descriptor),
        synthetic_usb_packet(device, 0x80, 2, irp_id, &request),
        synthetic_usb_packet(device, 0x80, 2, irp_id, &response),
        synthetic_usb_packet(0x63, 0x02, 1, [0xCC; 8], &[0xAA]),
    ]
}

fn hex_report(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&text[offset..offset + 2], 16).expect("hex fixture"))
        .collect()
}

fn append_u16(bytes: &mut Vec<u8>, value: u16, little_endian: bool) {
    let encoded = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    bytes.extend_from_slice(&encoded);
}

fn append_u32(bytes: &mut Vec<u8>, value: u32, little_endian: bool) {
    let encoded = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    bytes.extend_from_slice(&encoded);
}

fn append_pcapng_block(bytes: &mut Vec<u8>, block_type: u32, body: &[u8], little_endian: bool) {
    let block_len = 8 + body.len() + 4;
    append_u32(bytes, block_type, little_endian);
    append_u32(bytes, block_len as u32, little_endian);
    bytes.extend_from_slice(body);
    append_u32(bytes, block_len as u32, little_endian);
}

fn synthetic_classic_pcap(little_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if little_endian {
        bytes.extend_from_slice(&[0xD4, 0xC3, 0xB2, 0xA1]);
    } else {
        bytes.extend_from_slice(&[0xA1, 0xB2, 0xC3, 0xD4]);
    }
    append_u16(&mut bytes, 2, little_endian);
    append_u16(&mut bytes, 4, little_endian);
    append_u32(&mut bytes, 0, little_endian); // thiszone
    append_u32(&mut bytes, 0, little_endian); // sigfigs
    append_u32(&mut bytes, 65_535, little_endian); // snaplen
    append_u32(&mut bytes, 249, little_endian); // USBPcap link type
    append_u32(&mut bytes, 1, little_endian); // timestamp seconds
    append_u32(&mut bytes, 2, little_endian); // timestamp microseconds
    append_u32(&mut bytes, 3, little_endian); // captured length
    append_u32(&mut bytes, 5, little_endian); // original length
    bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
    bytes
}

fn synthetic_pcapng(little_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::new();

    let mut section = Vec::new();
    append_u32(&mut section, 0x1A2B3C4D, little_endian);
    append_u16(&mut section, 1, little_endian);
    append_u16(&mut section, 0, little_endian);
    append_u32(&mut section, u32::MAX, little_endian);
    append_u32(&mut section, u32::MAX, little_endian);
    append_pcapng_block(&mut bytes, 0x0A0D0D0A, &section, little_endian);

    let mut interface = Vec::new();
    append_u16(&mut interface, 249, little_endian);
    append_u16(&mut interface, 0, little_endian);
    append_u32(&mut interface, 65_535, little_endian);
    append_pcapng_block(&mut bytes, 0x00000001, &interface, little_endian);

    let mut packet = Vec::new();
    append_u32(&mut packet, 0, little_endian); // interface id
    append_u32(&mut packet, 0, little_endian); // timestamp high
    append_u32(&mut packet, 1, little_endian); // timestamp low
    append_u32(&mut packet, 3, little_endian); // captured length
    append_u32(&mut packet, 5, little_endian); // original length
    packet.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0x00]); // one pad byte
    append_pcapng_block(&mut bytes, 0x00000006, &packet, little_endian);
    bytes
}

#[test]
fn synthetic_classic_and_pcapng_parsers_preserve_endian_and_lengths() {
    for little_endian in [true, false] {
        let classic = parse_pcap(
            &synthetic_classic_pcap(little_endian),
            Path::new("synthetic-classic.pcap"),
        );
        assert_eq!(classic.len(), 1);
        assert_eq!(classic[0].data, [0xAA, 0xBB, 0xCC]);
        assert_eq!(classic[0].captured_len, 3);
        assert_eq!(classic[0].original_len, 5);

        let pcapng = parse_pcapng(
            &synthetic_pcapng(little_endian),
            Path::new("synthetic-capture.pcapng"),
        );
        assert_eq!(pcapng.len(), 1);
        assert_eq!(pcapng[0].data, [0xAA, 0xBB, 0xCC]);
        assert_eq!(pcapng[0].captured_len, 3);
        assert_eq!(pcapng[0].original_len, 5);
    }
}

#[test]
fn profile_evidence_hashes_pin_the_known_polling_rate_ab_pair() {
    let before_path = profile_evidence_path(PROFILE_BEFORE);
    let after_path = profile_evidence_path(PROFILE_AFTER);
    assert_profile_hash(&before_path, PROFILE_HASH_125_HZ);
    assert_profile_hash(&after_path, PROFILE_HASH_250_HZ);

    let before = fs::read(&before_path).expect("read 125 Hz profile evidence");
    let after = fs::read(&after_path).expect("read 250 Hz profile evidence");
    assert_ne!(before, after, "the known A/B profile pair must differ");
}

#[test]
fn future_reconnect_ab_contract_is_address_agnostic_and_exact() {
    let before_packets = synthetic_reconnect_readback_capture(7, POLLING_RATE_READBACK_125_HZ);
    let after_packets = synthetic_reconnect_readback_capture(3, POLLING_RATE_READBACK_250_HZ);

    let before_target = assert_reconnect_ab_snapshot(
        &before_packets,
        &profile_evidence_path(PROFILE_BEFORE),
        PROFILE_HASH_125_HZ,
        POLLING_RATE_READBACK_125_HZ,
    );
    let after_target = assert_reconnect_ab_snapshot(
        &after_packets,
        &profile_evidence_path(PROFILE_AFTER),
        PROFILE_HASH_250_HZ,
        POLLING_RATE_READBACK_250_HZ,
    );

    assert_eq!(before_target, 7);
    assert_eq!(after_target, 3);
}

#[test]
fn evidence_fixtures_are_present_lossless_and_targeted() {
    for &(name, _, _) in PCAPNG_EVIDENCE_FIXTURES {
        let packets = capture_packets(&capture_path(name));
        assert_exact_capture_lengths(&packets);
        assert_target_attribution(&packets);
    }

    let packets = capture_packets(&capture_path(CLASSIC_EVIDENCE_FIXTURE));
    assert_exact_capture_lengths(&packets);
    assert_target_attribution(&packets);

    let packets = capture_packets(&capture_path(RECONNECT_EVIDENCE_FIXTURE));
    assert_exact_capture_lengths(&packets);
    assert_target_attribution(&packets);
}

#[test]
fn target_report_routes_and_setups_match_each_evidence_fixture() {
    for &(name, expected_16, expected_64) in PCAPNG_EVIDENCE_FIXTURES {
        let packets = capture_packets(&capture_path(name));
        let target = assert_target_attribution(&packets);
        assert_target_report_routes(&packets, target, expected_16, expected_64);
    }
}

#[test]
fn every_captured_target_set_report_has_a_zero_status_irp_pair() {
    for &(name, expected_16, expected_64) in PCAPNG_EVIDENCE_FIXTURES {
        let packets = capture_packets(&capture_path(name));
        let target = assert_target_attribution(&packets);
        assert_set_report_completions(&packets, target);
        assert_eq!(
            set_report_records(&packets, target).len(),
            expected_16 + expected_64,
            "SET_REPORT count changed for {name}"
        );
    }

    let packets = capture_packets(&capture_path(CLASSIC_EVIDENCE_FIXTURE));
    let target = assert_target_attribution(&packets);
    assert_set_report_completions(&packets, target);
    assert!(set_report_records(&packets, target).is_empty());

    let packets = capture_packets(&capture_path(RECONNECT_EVIDENCE_FIXTURE));
    let target = assert_target_attribution(&packets);
    assert_set_report_completions(&packets, target);
    assert!(
        assert_get_report_readback_pairs(&packets, target).is_empty(),
        "reconnect evidence must not imply a target readback"
    );
    assert!(set_report_records(&packets, target).is_empty());
}

#[test]
fn unverified_observations_cannot_create_an_apply_plan_frame() {
    let plan = ApplyPlan::dry_run(&Profile::default_profile(1));
    assert!(
        plan.frames.is_empty(),
        "no speculative write frame is authorized"
    );
    assert!(
        plan.status_query().is_some(),
        "observed status stays informational"
    );
    assert!(
        !plan.warnings.is_empty(),
        "unverified fields must remain warnings"
    );
    assert!(
        plan.verified_frames().is_empty(),
        "unverified observations must not mint gated write tokens"
    );
    assert!(
        plan.phases().is_empty(),
        "an informational status query must not become an apply phase"
    );

    let mut sends = 0usize;
    let error = plan
        .apply_with(|_| {
            sends += 1;
            Ok::<(), &'static str>(())
        })
        .expect_err("a plan with unverified fields must fail closed");
    assert!(matches!(error, ApplyError::UnsupportedFields { .. }));
    assert_eq!(sends, 0, "fail-closed planning must not reach transport");

    let commit_plan = ApplyPlan::try_build_with_options(
        &Profile::default_profile(1),
        redsamurai_config::device_apply::ApplyOptions {
            include_status_query: true,
            include_commit: true,
        },
    )
    .expect("observed status query must remain buildable");
    assert!(
        commit_plan.frames.is_empty(),
        "an unverified F1/F5 commit must not create a speculative frame"
    );
    assert!(commit_plan
        .warnings
        .iter()
        .any(|warning| warning.field_name() == "Commit"));
}

#[test]
fn controlled_polling_rate_captures_preserve_the_single_delta() {
    assert_paired_captures(
        "polling_rate_1000_to_250_usbpcap2.pcapng",
        "polling_rate_250_to_1000_usbpcap2.pcapng",
    );
}

#[test]
fn additional_polling_rate_captures_preserve_the_second_single_delta() {
    assert_paired_captures_with_delta(
        "polling_rate_1000_to_500_usbpcap2.pcapng",
        "polling_rate_500_to_125_usbpcap2.pcapng",
        "02f33200060000000200080002000000",
        "02f33200060000000800080002000000",
    );
}

#[test]
fn light_tab_bursts_isolate_brightness_mode_and_palette_fields() {
    assert_apply_bursts(
        "ui_light_batch_usbpcap2.pcapng",
        5,
        &[
            (
                0,
                &[(
                    4,
                    "02f34f04010000000200000000000000",
                    "02f34f04010000000300000000000000",
                )],
            ),
            (
                1,
                &[(
                    3,
                    "02f3490406000000ff00000305010000",
                    "02f3490406000000ff00000100010000",
                )],
            ),
            (
                2,
                &[(
                    3,
                    "02f3490406000000ff00000100010000",
                    "02f349040600000026a2ea0100010000",
                )],
            ),
            (
                3,
                &[
                    (
                        3,
                        "02f349040600000026a2ea0100010000",
                        "02f3490406000000b900010305010000",
                    ),
                    (
                        4,
                        "02f34f04010000000300000000000000",
                        "02f34f04010000000200000000000000",
                    ),
                ],
            ),
        ],
    );
}

#[test]
fn dpi_bursts_isolate_stage_enable_and_value_fields() {
    assert_apply_bursts(
        "dpi_batch_usbpcap2.pcapng",
        4,
        &[
            (
                0,
                &[(
                    43,
                    "02f35c0005000000017c040000000000",
                    "02f35c0005000000007c040000000000",
                )],
            ),
            (
                1,
                &[(
                    23,
                    "02f34400050000000116000000000000",
                    "02f3440005000000013a000000000000",
                )],
            ),
            (
                2,
                &[(
                    43,
                    "02f35c0005000000007c040000000000",
                    "02f35c0005000000017c040000000000",
                )],
            ),
        ],
    );
}

#[test]
fn general_button_and_polling_bursts_preserve_observed_deltas() {
    assert_apply_bursts(
        "general_button_usbpcap2.pcapng",
        4,
        &[
            (
                0,
                &[(
                    54,
                    "02f38e00040000008200000000000000",
                    "02f38e00040000008400000000000000",
                )],
            ),
            (
                1,
                &[(
                    13,
                    "02f33200060000000800080002000000",
                    "02f33200060000000400080002000000",
                )],
            ),
            (
                2,
                &[(
                    13,
                    "02f33200060000000400080002000000",
                    "02f33200060000000800080002000000",
                )],
            ),
        ],
    );
}

#[test]
fn elevated_apply_readback_capture_contains_no_get_report() {
    let path = capture_path("readback_apply_correct_usbpcap2.pcapng");
    let packets = pcapng_packets(&path);
    let target = target_address(&packets);
    let reports = set_report_payloads(&packets, target);

    // The elevated, physically clicked Apply/profile path contains two full
    // 156-report bursts plus five interstitial writes.  It is a negative
    // readback observation: this path issued no HID GET_REPORT request.
    assert_eq!(reports.len(), 317);
    assert_apply_bursts(
        "readback_apply_correct_usbpcap2.pcapng",
        2,
        &[(
            0,
            &[(
                49,
                "02f32c00020000000400000000000000",
                "02f32c00020000000000000000000000",
            )],
        )],
    );
    assert_eq!(get_report_requests(&packets, target), 0);
    assert!(assert_get_report_readback_pairs(&packets, target).is_empty());
}

#[test]
fn uac_off_startup_and_apply_captures_contain_no_get_report() {
    let startup_path = capture_path("readback_uacoff_startup_usbpcap2.pcapng");
    let startup_packets = pcapng_packets(&startup_path);
    let startup_target = target_address(&startup_packets);
    assert!(set_report_payloads(&startup_packets, startup_target).is_empty());
    assert_eq!(get_report_requests(&startup_packets, startup_target), 0);
    assert_interrupt_completions(&startup_packets, startup_target, 40);
    assert!(assert_get_report_readback_pairs(&startup_packets, startup_target).is_empty());

    let apply_path = capture_path("readback_uacoff_apply_usbpcap2.pcapng");
    let packets = pcapng_packets(&apply_path);
    let target = target_address(&packets);
    assert_eq!(set_report_payloads(&packets, target).len(), 156);
    assert_apply_bursts("readback_uacoff_apply_usbpcap2.pcapng", 1, &[]);
    assert_eq!(get_report_requests(&packets, target), 0);
    assert_set_report_completions(&packets, target);
    assert!(assert_get_report_readback_pairs(&packets, target).is_empty());
}

#[test]
fn reenumeration_classic_pcap_is_read_only_and_keeps_interrupt_pairs() {
    let path = capture_path("hid_reenum_usbpcap2_20260909_131359_ca7d7c9e.pcap");
    let packets = capture_packets(&path);
    let target = target_address(&packets);
    assert_eq!(target, 2);
    assert_eq!(set_report_payloads(&packets, target).len(), 0);
    assert_eq!(get_report_requests(&packets, target), 0);

    assert_packet_length_histogram(
        &packets,
        &[(27, 64), (28, 1), (35, 64), (36, 3), (46, 1), (112, 1)],
    );
    assert_enumeration_control_records(&packets, target);
    assert_interrupt_completions(&packets, target, 64);
    assert!(assert_get_report_readback_pairs(&packets, target).is_empty());
}

#[test]
fn reconnect_capture_is_enumeration_only_with_exact_lengths_and_attribution() {
    let path = capture_path(RECONNECT_EVIDENCE_FIXTURE);
    let bytes = read_capture_bytes(&path);
    assert_eq!(
        bytes.len(),
        RECONNECT_EVIDENCE_BYTES,
        "reconnect evidence file length changed"
    );
    let packets = parse_pcap(&bytes, &path);
    assert_exact_capture_lengths(&packets);
    let target = assert_target_attribution(&packets);
    assert_reconnect_capture_shape(&packets, target);
}

#[test]
fn physical_reconnect_apply_fixture_is_lossless_and_descriptor_attributed() {
    let packets = assert_lossless_hardware_capture(
        HARDWARE_RECONNECT_FIXTURE,
        HARDWARE_RECONNECT_BYTES,
        HARDWARE_RECONNECT_RECORDS,
        HARDWARE_RECONNECT_SHA256,
    );
    let target = assert_target_attribution(&packets);
    assert_descriptor_attribution(&packets, target, HARDWARE_RECONNECT_DESCRIPTOR_COUNT);

    let target_packets = packets_for_target(&packets, target);
    assert_eq!(
        target_packets.len(),
        HARDWARE_RECONNECT_TARGET_PACKETS,
        "physical reconnect target packet attribution changed"
    );
    assert_packet_length_histogram(
        &target_packets,
        &[
            (27, 30),
            (28, 554),
            (32, 6),
            (35, 4),
            (36, 442),
            (37, 4),
            (39, 2),
            (40, 200),
            (41, 2),
            (43, 10),
            (46, 2),
            (52, 375),
            (62, 30),
            (68, 10),
            (75, 2),
            (86, 160),
            (95, 2),
            (100, 171),
            (112, 2),
            (134, 2),
        ],
    );

    let controls = target_control_records(&packets, target);
    assert_eq!(
        controls.len(),
        HARDWARE_RECONNECT_TARGET_CONTROLS,
        "physical reconnect target control-record count changed"
    );
    assert_target_control_status_histogram(&packets, target, &[(0, 1_972), (0xC0000004, 4)]);
    assert_hardware_enumeration_control_records(&packets, target);

    let set_reports = set_report_records(&packets, target);
    assert_eq!(
        set_reports.len(),
        HARDWARE_RECONNECT_SET_REPORTS,
        "physical reconnect SET_REPORT count changed"
    );
    assert_target_report_routes(&packets, target, 375, 171);
    assert_set_report_completions(&packets, target);
    assert_hardware_apply_burst(&packets, target);

    let readbacks = assert_get_report_readback_pairs(&packets, target);
    assert_hardware_reconnect_readback_shape(&readbacks);
    assert_hardware_profile_metadata(HARDWARE_PROFILE_BEFORE_METADATA, PROFILE_HASH_250_HZ);
    assert_hardware_profile_metadata(HARDWARE_PROFILE_AFTER_METADATA, PROFILE_HASH_125_HZ);
}

#[test]
fn physical_restart_readback_fixture_pairs_post_125_state_exactly() {
    let packets = assert_lossless_hardware_capture(
        HARDWARE_RESTART_FIXTURE,
        HARDWARE_RESTART_BYTES,
        HARDWARE_RESTART_RECORDS,
        HARDWARE_RESTART_SHA256,
    );
    let target = assert_target_attribution(&packets);
    assert_descriptor_attribution(&packets, target, HARDWARE_RESTART_DESCRIPTOR_COUNT);

    let target_packets = packets_for_target(&packets, target);
    assert_eq!(
        target_packets.len(),
        HARDWARE_RESTART_TARGET_PACKETS,
        "post-125 restart target packet attribution changed"
    );
    assert_packet_length_histogram(
        &target_packets,
        &[
            (28, 196),
            (36, 208),
            (37, 1),
            (39, 1),
            (40, 100),
            (41, 1),
            (43, 5),
            (46, 1),
            (52, 110),
            (62, 12),
            (68, 5),
            (86, 80),
            (100, 85),
            (112, 1),
        ],
    );

    let controls = target_control_records(&packets, target);
    assert_eq!(
        controls.len(),
        HARDWARE_RESTART_TARGET_CONTROLS,
        "post-125 restart target control-record count changed"
    );
    assert_target_control_status_histogram(&packets, target, &[(0, 806)]);
    assert_enumeration_control_records(&packets, target);

    let set_reports = set_report_records(&packets, target);
    assert_eq!(
        set_reports.len(),
        HARDWARE_RESTART_SET_REPORTS,
        "post-125 restart SET_REPORT count changed"
    );
    assert_target_report_routes(&packets, target, 110, 85);
    assert_set_report_completions(&packets, target);
    let readbacks = assert_get_report_readback_pairs(&packets, target);
    assert_eq!(
        readbacks.len(),
        HARDWARE_RESTART_GET_REPORTS,
        "post-125 restart GET_REPORT/readback count changed"
    );
    assert_restart_readback_shape(&readbacks, POLLING_RATE_READBACK_125_HZ);

    assert_hardware_profile_metadata(HARDWARE_PROFILE_BEFORE_METADATA, PROFILE_HASH_250_HZ);
    assert_hardware_profile_metadata(HARDWARE_PROFILE_AFTER_METADATA, PROFILE_HASH_125_HZ);
    assert_hardware_restart_profile_metadata();
}

#[test]
fn post_restart_capture_pairs_enumeration_set_and_readback_records() {
    let path = capture_path(RESTART_TRACE_FIXTURE);
    let bytes = read_capture_bytes(&path);
    assert_eq!(
        bytes.len(),
        RESTART_TRACE_BYTES,
        "post-restart evidence file length changed"
    );
    let packets = parse_pcap(&bytes, &path);
    assert_eq!(
        packets.len(),
        RESTART_TRACE_RECORDS,
        "post-restart evidence record count changed"
    );
    assert_exact_capture_lengths(&packets);

    let target = assert_target_attribution(&packets);
    let mut target_packets = Vec::new();
    for (index, packet) in packets.iter().enumerate() {
        let header = usb_header(&packet.data)
            .unwrap_or_else(|| panic!("post-restart packet {index} has no USBPcap header"));
        if header.device == target {
            assert_eq!(
                packet.captured_len,
                header.header_len + header.data_len,
                "post-restart target packet {index} length is not exact"
            );
            target_packets.push(packet.clone());
        }
    }
    assert_eq!(
        target_packets.len(),
        RESTART_TARGET_PACKETS,
        "post-restart target packet attribution changed"
    );
    assert_packet_length_histogram(
        &target_packets,
        &[
            (28, 196),
            (36, 208),
            (37, 1),
            (39, 1),
            (40, 100),
            (41, 1),
            (43, 5),
            (46, 1),
            (52, 110),
            (62, 12),
            (68, 5),
            (86, 80),
            (100, 85),
            (112, 1),
        ],
    );

    let controls = target_control_records(&packets, target);
    assert_eq!(
        controls.len(),
        RESTART_TARGET_PACKETS,
        "post-restart target control-record count changed"
    );
    assert_enumeration_control_records(&packets, target);
    let set_reports = set_report_records(&packets, target);
    assert_eq!(
        set_reports.len(),
        RESTART_SET_REPORTS,
        "post-restart SET_REPORT count changed"
    );
    assert_target_report_routes(&packets, target, 110, 85);
    assert_set_report_completions(&packets, target);
    let readbacks = assert_get_report_readback_pairs(&packets, target);
    assert_restart_readback_shape(&readbacks, POLLING_RATE_READBACK_250_HZ);
    assert_restart_profile_matches_readback(&readbacks);
}

#[test]
fn post_restart_ui_apply_capture_has_one_exact_write_burst_without_readback() {
    let path = capture_path(UI_APPLY_TRACE_FIXTURE);
    let bytes = read_capture_bytes(&path);
    assert_eq!(
        bytes.len(),
        UI_APPLY_TRACE_BYTES,
        "post-restart UI Apply evidence file length changed"
    );
    let packets = parse_pcap(&bytes, &path);
    assert_eq!(
        packets.len(),
        UI_APPLY_TRACE_RECORDS,
        "post-restart UI Apply evidence record count changed"
    );
    assert_exact_capture_lengths(&packets);

    let target = assert_target_attribution(&packets);
    let mut target_packets = Vec::new();
    for (index, packet) in packets.iter().enumerate() {
        let header = usb_header(&packet.data).unwrap_or_else(|| {
            panic!("post-restart UI Apply packet {index} has no USBPcap header")
        });
        if header.device == target {
            assert_eq!(
                packet.captured_len,
                header.header_len + header.data_len,
                "post-restart UI Apply target packet {index} length is not exact"
            );
            target_packets.push(packet.clone());
        }
    }
    assert_eq!(
        target_packets.len(),
        UI_APPLY_TARGET_PACKETS,
        "post-restart UI Apply target packet attribution changed"
    );
    assert_packet_length_histogram(
        &target_packets,
        &[(28, 157), (36, 3), (46, 1), (52, 155), (100, 1), (112, 1)],
    );

    let controls = target_control_records(&packets, target);
    assert_eq!(controls.len(), UI_APPLY_TARGET_PACKETS);
    assert_enumeration_control_records(&packets, target);
    let set_reports = set_report_records(&packets, target);
    assert_eq!(set_reports.len(), UI_APPLY_SET_REPORTS);
    assert_target_report_routes(&packets, target, 155, 1);
    assert_set_report_completions(&packets, target);
    assert!(
        assert_get_report_readback_pairs(&packets, target).is_empty(),
        "post-restart UI Apply evidence must not imply a readback"
    );
    assert_eq!(
        set_reports[13].report,
        hex_report("02f33200060000000400080002000000"),
        "post-restart UI Apply polling report changed"
    );
    assert_burst_shape(
        &set_reports
            .into_iter()
            .map(|record| record.report)
            .collect::<Vec<_>>(),
    );
    assert_ui_apply_profile_is_unchanged();
}

#[test]
fn official_gui_rainbow_apply_is_a_complete_ordered_burst() {
    let path = capture_path(LIGHT_RAINBOW_FIXTURE);
    let bytes = read_capture_bytes(&path);
    assert_eq!(bytes.len(), LIGHT_RAINBOW_BYTES);
    assert_eq!(sha256_hex(&bytes), LIGHT_RAINBOW_SHA256);

    let packets = parse_pcap(&bytes, &path);
    assert_exact_capture_lengths(&packets);
    let target = target_address(&packets);
    let reports = set_report_payloads(&packets, target);
    let starts: Vec<usize> = reports
        .iter()
        .enumerate()
        .filter_map(|(index, report)| {
            (report == &hex_report("02f50000000000000000000000000000")).then_some(index)
        })
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "the rainbow capture must contain one full Apply"
    );
    let burst = &reports[starts[0]..starts[0] + APPLY_SEQUENCE_FRAME_COUNT];
    assert_burst_shape(burst);
    assert_eq!(
        burst[APPLY_LIGHT_MODE_FRAME_INDEX],
        hex_report("02f3490406000000ff00000305010000")
    );
}

#[test]
fn profile_apply_and_report_delta_remain_explicitly_bounded() {
    assert_profile_apply_delta();
    assert_paired_captures(
        "polling_rate_1000_to_250_usbpcap2.pcapng",
        "polling_rate_250_to_1000_usbpcap2.pcapng",
    );
}
