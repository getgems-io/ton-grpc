use adnl_tcp::types::Int256;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Error, Ref};

/// ed25519_pubkey#8e81278a pubkey:bits256 = SigPubKey;  // 288 bits
/// certificate#4 temp_key:SigPubKey valid_since:uint32 valid_until:uint32 = Certificate;  // 356 bits
/// signed_certificate$_ certificate:Certificate certificate_signature:CryptoSignature
///   = SignedCertificate;  // 356+516 = 872 bits
/// chained_signature#f signed_cert:^SignedCertificate temp_key_signature:CryptoSignatureSimple
///   = CryptoSignature;   // 4+(356+516)+516 = 520 bits+ref (1392 bits total)
/// ed25519_signature#5 R:bits256 s:bits256 = CryptoSignatureSimple;  // 516 bits
/// _ CryptoSignatureSimple = CryptoSignature;
/// sig_pair$_ node_id_short:bits256 sign:CryptoSignature = CryptoSignaturePair;
///

/// ```tlb
/// ed25519_pubkey#8e81278a pubkey:bits256 = SigPubKey;  // 288 bits
/// ```
#[derive(Debug, Clone)]
pub struct SigPubKey {
    pub pubkey: Int256,
}

impl<'de> CellDeserialize<'de> for SigPubKey {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;
        if tag != 0x8e81278a {
            return Err(Error::custom(format!(
                "invalid SigPubKey tag: 0x{:08x}",
                tag
            )));
        }

        let pubkey = parser.unpack(())?;

        Ok(Self { pubkey })
    }
}

/// ```tlb
/// certificate#4 temp_key:SigPubKey valid_since:uint32 valid_until:uint32 = Certificate;  // 356 bits
/// ```
#[derive(Debug, Clone)]
pub struct Certificate {
    pub temp_key: SigPubKey,
    pub valid_since: u32,
    pub valid_until: u32,
}

impl<'de> CellDeserialize<'de> for Certificate {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(())?;
        if tag != 0x4 {
            return Err(Error::custom(format!(
                "invalid Certificate tag: 0x{:x}",
                tag
            )));
        }

        let temp_key = parser.parse(())?;
        let valid_since = parser.unpack(())?;
        let valid_until = parser.unpack(())?;

        Ok(Self {
            temp_key,
            valid_since,
            valid_until,
        })
    }
}

/// ```tlb
/// signed_certificate$_ certificate:Certificate certificate_signature:CryptoSignature
///   = SignedCertificate;  // 356+516 = 872 bits
/// ```
#[derive(Debug, Clone)]
pub struct SignedCertificate {
    pub certificate: Certificate,
    pub certificate_signature: Box<CryptoSignature>,
}

impl<'de> CellDeserialize<'de> for SignedCertificate {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let certificate = parser.parse(())?;
        let certificate_signature = parser.parse(())?;

        Ok(Self {
            certificate,
            certificate_signature: Box::new(certificate_signature),
        })
    }
}

/// ```tlb
/// ed25519_signature#5 R:bits256 s:bits256 = CryptoSignatureSimple;  // 516 bits
/// ```
#[derive(Debug, Clone)]
pub struct CryptoSignatureSimple {
    pub r: Int256,
    pub s: Int256,
}

impl<'de> CellDeserialize<'de> for CryptoSignatureSimple {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(())?;
        if tag != 0x5 {
            return Err(Error::custom(format!(
                "invalid CryptoSignatureSimple tag: 0x{:x}",
                tag
            )));
        }

        let r = parser.unpack(())?;
        let s = parser.unpack(())?;

        Ok(Self { r, s })
    }
}

/// ```tlb
/// _ CryptoSignatureSimple = CryptoSignature;
/// chained_signature#f signed_cert:^SignedCertificate temp_key_signature:CryptoSignatureSimple
///   = CryptoSignature;
/// ```
#[derive(Debug, Clone)]
pub enum CryptoSignature {
    Simple(CryptoSignatureSimple),
    Chained {
        signed_cert: Box<SignedCertificate>,
        temp_key_signature: CryptoSignatureSimple,
    },
}

impl<'de> CellDeserialize<'de> for CryptoSignature {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(())?;

        match tag {
            0x5 => {
                let r = parser.unpack(())?;
                let s = parser.unpack(())?;

                Ok(Self::Simple(CryptoSignatureSimple { r, s }))
            }
            0xf => {
                let signed_cert = parser.parse_as::<SignedCertificate, Ref>(())?;
                let temp_key_signature = parser.parse(())?;

                Ok(Self::Chained {
                    signed_cert: Box::new(signed_cert),
                    temp_key_signature,
                })
            }
            _ => Err(Error::custom(format!(
                "invalid CryptoSignature tag: 0x{:x}",
                tag
            ))),
        }
    }
}

/// ```tlb
/// sig_pair$_ node_id_short:bits256 sign:CryptoSignature = CryptoSignaturePair;
/// ```
#[derive(Debug, Clone)]
pub struct CryptoSignaturePair {
    pub node_id_short: Int256,
    pub sign: CryptoSignature,
}

impl<'de> CellDeserialize<'de> for CryptoSignaturePair {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let node_id_short = parser.unpack(())?;
        let sign = parser.parse(())?;

        Ok(Self {
            node_id_short,
            sign,
        })
    }
}
