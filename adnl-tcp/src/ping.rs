use crate::packet::Packet;
use rand::random;

pub fn ping_packet() -> Packet {
    let nonce = random::<u64>();
    let crc32 = [0x9a, 0x2b, 0x08, 0x4d];

    Packet::new([&crc32, nonce.to_le_bytes().as_slice()].concat())
}

pub fn is_ping_packet(packet: &Packet) -> bool {
    packet.data.len() == 12 && packet.data.starts_with(&[0x9a, 0x2b, 0x08, 0x4d])
}

pub fn is_pong_packet(packet: &Packet) -> bool {
    packet.data.len() == 12 && packet.data.starts_with(&[0x03, 0xFB, 0x69, 0xDC])
}
