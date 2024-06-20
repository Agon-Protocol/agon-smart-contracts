use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};
use cw_utils::Expiration;

use crate::state::CompetitionType;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_ownable::cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    CreateCompetition {
        /// Override the minimum members for the competition
        min_members: Option<Uint128>,
        max_members: Uint128,
        /// The entry fee of the competition
        entry_fee: Option<Coin>,
        expiration: Expiration,
        category_id: Option<Uint128>,
        competition_info: CompetitionInfoMsg,
        /// Is the creator a member on creation
        /// Defaults to false
        is_creator_member: Option<bool>,
        rulesets: Vec<Uint128>,
        rules: Vec<String>,
    },
}

#[cw_serde]
pub struct CompetitionInfoMsg {
    pub name: String,
    pub description: String,
    pub expiration: Expiration,
    pub rules: Vec<String>,
    pub rulesets: Vec<Uint128>,
    pub banner: Option<String>,
    pub competition_type: CompetitionType,
}

#[cw_serde]
pub enum EnrollmentFilter {
    Expiration {},
    Host(String),
}

#[cw_ownable::cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {}

#[cw_serde]
pub enum MigrateMsg {
    FromCompatible {},
}
