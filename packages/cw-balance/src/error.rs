use cosmwasm_std::{
    CheckedFromRatioError, CheckedMultiplyFractionError, CoinsError, DecimalRangeExceeded,
    OverflowError, StdError,
};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum BalanceError {
    #[error("{0}")]
    StdError(#[from] StdError),

    #[error("{0}")]
    CheckedFromRatioError(#[from] CheckedFromRatioError),

    #[error("{0}")]
    CoinsError(#[from] CoinsError),

    #[error("{0}")]
    CheckedMultiplyFractionError(#[from] CheckedMultiplyFractionError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),
}
