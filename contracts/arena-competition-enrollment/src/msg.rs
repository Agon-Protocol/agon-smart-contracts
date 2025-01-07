use arena_interface::{competition::msg::EscrowContractInfo, group::MemberMsg};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Timestamp, Uint128, Uint64};
use dao_interface::state::ModuleInstantiateInfo;

use crate::state::{CompetitionType, EnrollmentEntryResponse};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
}

#[allow(clippy::large_enum_variant)]
#[cw_ownable::cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    #[cw_orch(payable)]
    CreateEnrollment {
        /// Override the minimum members for the competition
        min_members: Option<Uint64>,
        max_members: Uint64,
        /// The entry fee of the competition
        entry_fee: Option<Coin>,
        /// Seconds before the competition date until registration is expired
        duration_before: u64,
        category_id: Option<Uint128>,
        competition_info: CompetitionInfoMsg,
        competition_type: CompetitionType,
        group_contract_info: ModuleInstantiateInfo,
        required_team_size: Option<u32>,
        escrow_contract_info: EscrowContractInfo,
    },
    Finalize {
        id: Uint128,
    },
    #[cw_orch(payable)]
    Enroll {
        id: Uint128,
        /// Optional team to enroll
        /// Only callable by a member
        team: Option<String>,
    },
    Withdraw {
        id: Uint128,
    },
    ForceWithdraw {
        id: Uint128,
        members: Vec<String>,
    },
    SetRankings {
        id: Uint128,
        rankings: Vec<MemberMsg<String>>,
    },
}

#[cw_serde]
pub struct CompetitionInfoMsg {
    pub name: String,
    pub description: String,
    pub date: Timestamp,
    pub duration: u64,
    pub rules: Option<Vec<String>>,
    pub rulesets: Option<Vec<Uint128>>,
    pub banner: Option<String>,
}

#[cw_serde]
pub enum EnrollmentFilter {
    Category { category_id: Option<Uint128> },
    Host(String),
}

#[cw_ownable::cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(Vec<EnrollmentEntryResponse>)]
    Enrollments {
        start_after: Option<Uint128>,
        limit: Option<u32>,
        filter: Option<EnrollmentFilter>,
    },
    #[returns(EnrollmentEntryResponse)]
    Enrollment { enrollment_id: Uint128 },
    #[returns(Uint128)]
    EnrollmentCount {},
    #[returns(bool)]
    IsMember {
        enrollment_id: Uint128,
        addr: String,
    },
}

#[cw_serde]
pub enum MigrateMsg {
    FromCompatible {},
    RemoveThirdPlaceMatch { enrollment_id: Uint128 },
    FromV2_3 {},
}

#[cw_serde]
pub struct SudoMsg {
    pub enrollment_entry_response: EnrollmentEntryResponse,
}
