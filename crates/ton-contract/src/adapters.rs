use base64::{Engine, engine::general_purpose::STANDARD};
use core::{fmt::Display, str::FromStr};
use std::{error::Error as StdError, sync::Arc};
use ton_client::StackEntry;
use toner::{
    tlb::bits::de::unpack_bytes,
    tlb::bits::ser::pack,
    tlb::de::CellDeserializeAsOwned,
    tlb::de::CellDeserializeOwned,
    tlb::ser::{CellSerialize, CellSerializeAs, CellSerializeExt, CellSerializeWrapAsExt},
    tlb::{BagOfCellsArgs, BoC, Cell, Error as TlbError},
};

use crate::TonContractError;

pub trait StackEntryExt: Sized {
    fn to_boc(&self) -> Result<BoC, TonContractError>;
    #[inline]
    fn to_cell(&self) -> Result<Arc<Cell>, TonContractError> {
        self.to_boc()?
            .single_root()
            .ok_or_else(|| TonContractError::TLB(TlbError::custom("single root")))
            .cloned()
    }
    #[inline]
    fn parse_cell_fully<T>(&self) -> Result<T, TonContractError>
    where
        T: CellDeserializeOwned<Args = ()>,
    {
        self.to_cell()?.parse_fully(()).map_err(Into::into)
    }
    #[inline]
    fn parse_cell_fully_as<T, As>(&self) -> Result<T, TonContractError>
    where
        As: CellDeserializeAsOwned<T, Args = ()>,
    {
        self.to_cell()?
            .parse_fully_as::<T, As>(())
            .map_err(Into::into)
    }

    fn from_boc(boc: BoC) -> Result<Self, TonContractError>;
    #[inline]
    fn from_cell(cell: impl Into<Arc<Cell>>) -> Result<Self, TonContractError> {
        Self::from_boc(BoC::from_root(cell))
    }
    #[inline]
    fn store_cell<T>(value: T) -> Result<Self, TonContractError>
    where
        T: CellSerialize<Args = ()>,
    {
        Self::from_cell(value.to_cell(())?)
    }
    fn store_cell_as<T, As>(value: T) -> Result<Self, TonContractError>
    where
        As: CellSerializeAs<T, Args = ()>,
    {
        Self::from_cell(value.wrap_as::<As>().to_cell(())?)
    }

    fn to_number<T>(&self) -> Result<T, TonContractError>
    where
        T: FromStr,
        T::Err: StdError + Send + Sync + 'static;
    fn from_number<T>(number: T) -> Self
    where
        T: Display;
}

impl StackEntryExt for StackEntry {
    fn to_boc(&self) -> Result<BoC, TonContractError> {
        let bytes = match self {
            Self::Slice { bytes } | Self::Cell { bytes } => bytes,
            _ => return Err(TonContractError::InvalidStack),
        };

        let bytes = STANDARD.decode(bytes)?;

        unpack_bytes(&bytes, ()).map_err(Into::into)
    }

    fn from_boc(boc: BoC) -> Result<Self, TonContractError> {
        Ok(Self::Slice {
            bytes: STANDARD.encode(
                pack(
                    boc,
                    BagOfCellsArgs {
                        has_idx: false,
                        has_crc32c: false,
                    },
                )?
                .as_raw_slice(),
            ),
        })
    }

    fn to_number<T>(&self) -> Result<T, TonContractError>
    where
        T: FromStr,
        T::Err: Display,
    {
        let Self::Number { number } = self else {
            return Err(TonContractError::InvalidStack);
        };

        T::from_str(number).map_err(|err| TonContractError::ParseNumber(err.to_string()))
    }

    fn from_number<T>(number: T) -> Self
    where
        T: Display,
    {
        Self::Number {
            number: number.to_string(),
        }
    }
}
