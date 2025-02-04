use cosmwasm_std::{
    CheckedFromRatioError, Coin, DecimalRangeExceeded, Instantiate2AddressError, OverflowError,
    StdError, Uint64,
};
use cw_ownable::OwnershipError;
use cw_utils::{ParseReplyError, PaymentError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    StdError(#[from] StdError),

    #[error("{0}")]
    OwnershipError(#[from] OwnershipError),

    #[error("{0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),

    #[error("{0}")]
    CheckedFromRatioError(#[from] CheckedFromRatioError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("{0}")]
    Instantiate2AddressError(#[from] Instantiate2AddressError),

    #[error("Unknown reply ID {id}")]
    UnknownReplyId { id: u64 },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Already enrolled")]
    AlreadyEnrolled {},

    #[error("Cannot finalize with {current_members}/{max_members} members before expiration")]
    FinalizeFailed {
        max_members: Uint64,
        current_members: Uint64,
    },

    #[error("Competition has already been finalized")]
    AlreadyFinalized {},

    #[error("Entry fee {entry_fee} was not paid")]
    EntryFeeNotPaid { entry_fee: Coin },

    #[error("Not enrolled")]
    NotEnrolled {},

    #[error("Enrollment is at max members already")]
    EnrollmentMaxMembers {},

    #[error("Only teams of size {required_team_size} can enroll")]
    TeamSizeMismatch { required_team_size: u32 },

    #[error("Cannot enroll a team you are not a member of")]
    NotTeamMember {},
}
