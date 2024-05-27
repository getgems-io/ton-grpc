use core::{fmt::Display, str::FromStr};
use std::{error::Error as StdError, sync::Arc};

use base64::{engine::general_purpose::STANDARD, Engine};
use toner::tlb::{
    ton::BoC, unpack_bytes, Cell, CellDeserializeAsOwned, CellDeserializeOwned, CellSerialize,
    CellSerializeAs, CellSerializeExt, CellSerializeWrapAsExt, Error as TlbError,
};

use tonlibjson_client::block::{
    TvmBoxedNumber, TvmBoxedStackEntry, TvmCell, TvmNumberDecimal, TvmSlice, TvmStackEntryCell,
    TvmStackEntryNumber, TvmStackEntrySlice,
};

use crate::TonContractError;

pub trait TvmBoxedStackEntryExt: Sized {
    fn into_boc(&self) -> Result<BoC, TonContractError>;
    #[inline]
    fn into_cell(&self) -> Result<Arc<Cell>, TonContractError> {
        self.into_boc()?
            .single_root()
            .ok_or_else(|| TonContractError::TLB(TlbError::custom("single root")))
            .cloned()
    }
    #[inline]
    fn parse_cell_fully<T>(&self) -> Result<T, TonContractError>
    where
        T: CellDeserializeOwned,
    {
        self.into_cell()?.parse_fully().map_err(Into::into)
    }
    #[inline]
    fn parse_cell_fully_as<T, As>(&self) -> Result<T, TonContractError>
    where
        As: CellDeserializeAsOwned<T>,
    {
        self.into_cell()?
            .parse_fully_as::<T, As>()
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
        T: CellSerialize,
    {
        Self::from_cell(value.to_cell()?)
    }
    fn store_cell_as<T, As>(value: T) -> Result<Self, TonContractError>
    where
        As: CellSerializeAs<T>,
    {
        Self::from_cell(value.wrap_as::<As>().to_cell()?)
    }

    fn into_number<T>(&self) -> Result<T, TonContractError>
    where
        T: FromStr,
        T::Err: StdError + Send + Sync + 'static;
    fn from_number<T>(number: T) -> Self
    where
        T: Display;
}

impl TvmBoxedStackEntryExt for TvmBoxedStackEntry {
    fn into_boc(&self) -> Result<BoC, TonContractError> {
        let bytes = match self {
            Self::TvmStackEntrySlice(TvmStackEntrySlice {
                slice: TvmSlice { bytes },
            })
            | Self::TvmStackEntryCell(TvmStackEntryCell {
                cell: TvmCell { bytes },
            }) => bytes,
            _ => return Err(TonContractError::InvalidStack),
        };

        let bytes = STANDARD.decode(bytes)?;

        unpack_bytes(bytes).map_err(Into::into)
    }

    fn from_boc(boc: BoC) -> Result<Self, TonContractError> {
        Ok(Self::TvmStackEntrySlice(TvmStackEntrySlice {
            slice: TvmSlice {
                bytes: STANDARD.encode(boc.pack(true)?),
            },
        }))
    }

    fn into_number<T>(&self) -> Result<T, TonContractError>
    where
        T: FromStr,
        T::Err: Display,
    {
        let Self::TvmStackEntryNumber(TvmStackEntryNumber {
            number: TvmBoxedNumber { number },
        }) = self
        else {
            return Err(TonContractError::InvalidStack);
        };

        T::from_str(number).map_err(|err| TonContractError::ParseNumber(err.to_string()))
    }
    fn from_number<T>(number: T) -> Self
    where
        T: Display,
    {
        Self::TvmStackEntryNumber(TvmStackEntryNumber {
            number: TvmNumberDecimal {
                number: number.to_string(),
            },
        })
    }
}
