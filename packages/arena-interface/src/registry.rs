use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use cw_balance::{Distribution, MemberPercentage};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    SetDistribution {
        distribution: Distribution<String>,
    },
    SetDistributionRemainderSelf {
        member_percentages: Vec<MemberPercentage<String>>,
    },
    RemoveDistribution {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(Option<Distribution<Addr>>)]
    GetDistribution { addr: String, height: Option<u64> },
    #[returns(Vec<(Addr, Distribution<Addr>)>)]
    GetDistributions {
        addrs: Vec<String>,
        height: Option<u64>,
    },
}

#[cw_serde]
pub enum MigrateMsg {
    FromCompatible {},
}
