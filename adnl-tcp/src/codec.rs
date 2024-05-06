use aes::cipher::generic_array::GenericArray;
use anyhow::bail;
use ctr::cipher::StreamCipher;
use ctr::cipher::KeyIvInit;
use sha2::{Digest, Sha256};
use tokio_util::bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use crate::aes_ctr::{Aes256Ctr128, AesCtr};
use crate::packet::Packet;

pub struct PacketCodec {
    cipher_recv: Aes256Ctr128,
    cipher_send: Aes256Ctr128,
    next_len: Option<usize>
}

impl Encoder<Packet> for PacketCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, packet: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let buf_size = dst.len();
        let len = packet.len();
        dst.reserve(len + 68);

        dst.put(&((len + 64) as u32).to_le_bytes()[..]);

        dst.put(&packet.nonce[..]);
        dst.put(&packet.data[..]);
        dst.put(&packet.checksum[..]);

        self.cipher_send.apply_keystream(&mut dst[buf_size .. ]);

        Ok(())
    }
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let length = match self.next_len.take() {
            Some(len) => { len }
            None => {
                if src.len() < 4 {
                    return Ok(None);
                }

                let mut length = src.split_to(4);
                self.cipher_recv.apply_keystream(&mut length);

                u32::from_le_bytes([ length[0], length[1], length[2], length[3] ]) as usize
            }
        };

        if length < 64 {
            bail!("small ADNL packet: {}", length);
        }

        if src.len() < length {
            src.reserve(length);
            self.next_len.replace(length);

            return Ok(None);
        }

        self.cipher_recv.apply_keystream(&mut src[0 .. length]);
        let sha256: [u8; 32] = Sha256::digest(&src[..length - 32]).into();
        if sha256 != src[length - 32..length] {
            bail!("incorrect checksum for ADNL packet");
        }

        let data = src.split_to(length);
        let packet = Packet {
            nonce: data[0 .. 32].try_into()?,
            data: data[32 .. length - 32].to_vec(),
            checksum: data[length - 32 .. length].try_into()?
        };

        src.reserve(68); // min size of packet is 68

        Ok(Some(packet))
    }
}

impl PacketCodec {
    pub fn from_aes_ctr_as_client(aes_ctr: AesCtr) -> Self {
        let bytes = aes_ctr.into_bytes();

        Self::from_bytes_as_client(&bytes)
    }

    pub fn from_aes_ctr_as_server(aes_ctr: AesCtr) -> Self {
        let bytes = aes_ctr.into_bytes();

        Self::from_bytes_as_server(&bytes)
    }

    fn from_bytes_as_client(bytes: &[u8; 160]) -> Self {
        let cipher_recv = Aes256Ctr128::new(GenericArray::from_slice(&bytes[0..32]), GenericArray::from_slice(&bytes[64 .. 80]));
        let cipher_send = Aes256Ctr128::new(GenericArray::from_slice(&bytes[32..64]), GenericArray::from_slice(&bytes[80 .. 96]));

        Self { cipher_recv, cipher_send, next_len: None }
    }

    fn from_bytes_as_server(bytes: &[u8; 160]) -> Self {
        let cipher_recv = Aes256Ctr128::new(GenericArray::from_slice(&bytes[32..64]), GenericArray::from_slice(&bytes[80 .. 96]));
        let cipher_send = Aes256Ctr128::new(GenericArray::from_slice(&bytes[0..32]), GenericArray::from_slice(&bytes[64 .. 80]));

        Self { cipher_recv, cipher_send, next_len: None }
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::bytes::{BufMut, BytesMut};
    use tokio_util::codec::{Decoder, Encoder};
    use tracing_test::traced_test;
    use anyhow::Result;
    use crate::codec::PacketCodec;
    use crate::packet::Packet;

    #[test]
    #[traced_test]
    fn encode_empty_packet() -> Result<()> {
        let mut codec = given_codec_server();
        let packet = empty_packet();

        let mut buf = BytesMut::new();
        codec.encode(packet, &mut buf)?;

        assert_eq!(buf.to_vec(), empty_packet_bytes());

        Ok(())
    }

    #[test]
    #[traced_test]
    fn decode_empty_packet() -> Result<()> {
        let mut codec = given_codec_client();
        let data = empty_packet_bytes();
        let mut buf = BytesMut::with_capacity(68);
        buf.put(&data[..]);

        let packet = codec.decode(&mut buf)?.unwrap();

        assert_eq!(packet, empty_packet());

        Ok(())
    }

    #[test]
    #[traced_test]
    fn decode_empty_packet_partial() -> Result<()> {
        let mut codec = given_codec_client();
        let data = empty_packet_bytes();
        let mut buf = BytesMut::with_capacity(68);
        buf.put(&data[.. 4]);
        let _ = codec.decode(&mut buf)?;

        buf.put(&data[4 ..]);
        let packet = codec.decode(&mut buf)?.unwrap();

        assert_eq!(packet, empty_packet());

        Ok(())
    }

    fn empty_packet() -> Packet {
        Packet {
            nonce: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
            data: vec![],
            checksum: [174, 33, 108, 46, 245, 36, 122, 55, 130, 193, 53, 239, 162, 121, 163, 228, 205, 198, 16, 148, 39, 15, 93, 43, 229, 140, 98, 4, 183, 166, 18, 201],
        }
    }

    fn empty_packet_bytes() -> Vec<u8> {
        vec![236, 54, 122, 182, 214, 17, 172, 214, 162, 205, 134, 120, 170, 101, 249, 104, 117, 236, 29, 70, 212, 118, 65, 112, 186, 83, 233, 46, 171, 214, 143, 155, 205, 253, 189, 157, 156, 69, 198, 77, 204, 137, 167, 26, 135, 32, 12, 15, 136, 91, 210, 94, 248, 123, 235, 232, 40, 156, 113, 185, 39, 169, 111, 2, 228, 61, 170, 25]
    }

    fn given_codec_server() -> PacketCodec {
        PacketCodec::from_bytes_as_server(&[222, 151, 216, 98, 74, 67, 129, 33, 184, 106, 25, 86, 84, 75, 215, 46, 214, 140, 214, 159, 44, 153, 85, 91, 8, 177, 232, 197, 31, 253, 81, 28, 55, 250, 129, 200, 76, 205, 84, 124, 48, 193, 118, 177, 24, 213, 203, 137, 43, 219, 17, 62, 142, 128, 20, 31, 38, 101, 25, 66, 46, 249, 238, 253, 134, 37, 18, 162, 54, 61, 178, 179, 163, 117, 192, 212, 187, 189, 39, 23, 33, 128, 216, 159, 35, 242, 226, 89, 186, 200, 80, 171, 2, 97, 147, 1, 151, 110, 92, 63, 166, 32, 9, 44, 113, 141, 133, 44, 167, 3, 182, 218, 158, 48, 117, 185, 242, 236, 184, 237, 66, 217, 247, 70, 191, 38, 170, 251, 127, 138, 50, 85, 4, 231, 49, 94, 218, 153, 125, 183, 134, 28, 148, 71, 245, 195, 239, 242, 99, 51, 178, 1, 128, 71, 93, 148, 68, 58, 16, 198])
    }

    fn given_codec_client() -> PacketCodec {
        PacketCodec::from_bytes_as_client(&[222, 151, 216, 98, 74, 67, 129, 33, 184, 106, 25, 86, 84, 75, 215, 46, 214, 140, 214, 159, 44, 153, 85, 91, 8, 177, 232, 197, 31, 253, 81, 28, 55, 250, 129, 200, 76, 205, 84, 124, 48, 193, 118, 177, 24, 213, 203, 137, 43, 219, 17, 62, 142, 128, 20, 31, 38, 101, 25, 66, 46, 249, 238, 253, 134, 37, 18, 162, 54, 61, 178, 179, 163, 117, 192, 212, 187, 189, 39, 23, 33, 128, 216, 159, 35, 242, 226, 89, 186, 200, 80, 171, 2, 97, 147, 1, 151, 110, 92, 63, 166, 32, 9, 44, 113, 141, 133, 44, 167, 3, 182, 218, 158, 48, 117, 185, 242, 236, 184, 237, 66, 217, 247, 70, 191, 38, 170, 251, 127, 138, 50, 85, 4, 231, 49, 94, 218, 153, 125, 183, 134, 28, 148, 71, 245, 195, 239, 242, 99, 51, 178, 1, 128, 71, 93, 148, 68, 58, 16, 198])
    }
}
