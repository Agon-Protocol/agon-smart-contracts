use cosmwasm_std::{CheckedFromRatioError, DecimalRangeExceeded, OverflowError, StdError};
use cw_controllers::AdminError;
use cw_utils::ParseReplyError;
use thiserror::Error;

use crate::state::CompetitionStatus;

#[derive(Error, Debug, PartialEq)]
pub enum CompetitionError {
    #[error("{0}")]
    StdError(#[from] StdError),

    #[error("{0}")]
    AdminError(#[from] AdminError),

    #[error("{0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),

    #[error("{0}")]
    CheckedFromRatioError(#[from] CheckedFromRatioError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("UnknownCompetitionId")]
    UnknownCompetitionId { id: u128 },

    #[error("CompetitionNotExpired")]
    CompetitionNotExpired {},

    #[error("UnknownReplyId")]
    UnknownReplyId { id: u64 },

    #[error("InvalidCompetitionStatus")]
    InvalidCompetitionStatus { current_status: CompetitionStatus },

    #[error("AttributeNotFound")]
    AttributeNotFound { key: String },
}
