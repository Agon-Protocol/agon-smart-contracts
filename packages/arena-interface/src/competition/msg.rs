use std::marker::PhantomData;

#[allow(unused_imports)]
use crate::competition::state::{CompetitionResponse, CompetitionStatus, Config, Evidence};
use crate::{
    fees::FeeInformation,
    group::{self},
};
use cosmwasm_schema::{cw_serde, schemars::JsonSchema, QueryResponses};
use cosmwasm_std::{Addr, Binary, Deps, StdResult, Timestamp, Uint128};
use cw_balance::Distribution;
use cw_ownable::{cw_ownable_execute, cw_ownable_query};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::stats::{MemberStatsMsg, StatMsg, StatTableEntry, StatType};

#[cw_serde]
pub struct InstantiateBase<InstantiateExt> {
    pub key: String, //this is used to map a key (wager, tournament, league) to a module
    pub description: String,
    pub extension: InstantiateExt,
}

#[cw_ownable_execute]
#[cw_serde]
#[allow(clippy::large_enum_variant)]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteBase<ExecuteExt, CompetitionInstantiateExt> {
    #[cw_orch(payable)]
    JailCompetition {
        competition_id: Uint128,
        title: String,
        description: String,
        distribution: Option<Distribution<String>>,
    },
    ActivateCompetition {},
    CreateCompetition {
        /// The competition's host
        /// Defaults to info.sender
        /// This can only be overridden by valid competition enrollment modules
        host: Option<String>,
        category_id: Option<Uint128>,
        escrow: EscrowContractInfo,
        name: String,
        description: String,
        date: Timestamp,
        /// Seconds after date that the competition is considered expired
        duration: u64,
        rules: Option<Vec<String>>,
        rulesets: Option<Vec<Uint128>>,
        banner: Option<String>,
        group_contract: group::GroupContractInfo,
        instantiate_extension: CompetitionInstantiateExt,
    },
    SubmitEvidence {
        competition_id: Uint128,
        evidence: Vec<String>,
    },
    ProcessCompetition {
        competition_id: Uint128,
        distribution: Option<Distribution<String>>,
    },
    Extension {
        msg: ExecuteExt,
    },
    MigrateEscrows {
        start_after: Option<Uint128>,
        limit: Option<u32>,
        filter: Option<CompetitionsFilter>,
        escrow_code_id: u64,
        escrow_migrate_msg: crate::escrow::MigrateMsg,
    },
    InputStats {
        competition_id: Uint128,
        stats: Vec<MemberStatsMsg>,
    },
    UpdateStatTypes {
        competition_id: Uint128,
        to_add: Vec<StatType>,
        to_remove: Vec<String>,
    },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryBase<InstantiateExt, QueryExt, CompetitionExt>
where
    InstantiateExt: Serialize + std::fmt::Debug + DeserializeOwned,
    QueryExt: JsonSchema,
    CompetitionExt: Serialize + std::fmt::Debug + DeserializeOwned,
{
    #[returns(Config<InstantiateExt>)]
    Config {},
    #[returns(String)]
    DAO {},
    #[returns(Uint128)]
    CompetitionCount {},
    #[returns(CompetitionResponse<CompetitionExt>)]
    Competition { competition_id: Uint128 },
    #[returns(Vec<CompetitionResponse<CompetitionExt>>)]
    Competitions {
        start_after: Option<Uint128>,
        limit: Option<u32>,
        filter: Option<CompetitionsFilter>,
    },
    #[returns(Vec<Evidence>)]
    Evidence {
        competition_id: Uint128,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Option<Distribution<String>>)]
    Result { competition_id: Uint128 },
    #[returns(cosmwasm_std::Binary)]
    QueryExtension { msg: QueryExt },
    #[returns(Option<String>)]
    PaymentRegistry {},
    #[returns(Option<Vec<StatType>>)]
    StatTypes { competition_id: Uint128 },
    /// Returns a user's historical stats for a competition
    #[returns(Vec<Vec<StatMsg>>)]
    HistoricalStats {
        competition_id: Uint128,
        addr: String,
    },
    /// Returns all current stats for a competition
    #[returns(Vec<StatTableEntry>)]
    StatsTable {
        competition_id: Uint128,
        start_after: Option<(String, String)>,
        limit: Option<u32>,
    },
    #[returns(StatMsg)]
    Stat {
        competition_id: Uint128,
        addr: String,
        stat_name: String,
        height: Option<u64>,
    },
    #[serde(skip)]
    #[returns(PhantomData<(InstantiateExt, CompetitionExt)>)]
    _Phantom(PhantomData<(InstantiateExt, CompetitionExt)>),
}

#[cw_serde]
pub enum MigrateBase {
    FromCompatible {},
    FromV2_3 {},
}

#[cw_serde]
pub enum EscrowContractInfo {
    Existing {
        addr: String,
        additional_layered_fees: Option<Vec<FeeInformation<String>>>,
    },
    New {
        /// Code ID of the contract to be instantiated.
        code_id: u64,
        /// Instantiate message to be used to create the contract.
        msg: Binary,
        /// Label for the instantiated contract.
        label: String,
        /// Optional additional layered fees
        additional_layered_fees: Option<Vec<FeeInformation<String>>>,
    },
}

#[cw_serde]
pub enum CompetitionsFilter {
    CompetitionStatus { status: CompetitionStatus },
    Category { id: Option<Uint128> },
    Host(String),
}

pub trait ToCompetitionExt<T> {
    fn to_competition_ext(&self, deps: Deps, group_contract: &Addr) -> StdResult<T>;
}
