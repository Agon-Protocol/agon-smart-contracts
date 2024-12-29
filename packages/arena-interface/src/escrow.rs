use crate::fees::FeeInformation;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;
use cw20::Cw20ReceiveMsg;
use cw_balance::{Assets, MemberAssets, MemberAssetsUnchecked};
#[allow(unused_imports)]
use cw_balance::{
    BalanceVerified, Distribution, MemberBalanceChecked, MemberBalanceUnchecked, MemberPercentage,
};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

#[cw_serde]
pub struct InstantiateMsg {
    pub dues: Vec<MemberAssetsUnchecked>,
}

#[cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    Withdraw {
        cw20_msg: Option<Binary>,
        cw721_msg: Option<Binary>,
    },
    #[cw_orch(payable)]
    ReceiveNative {},
    Receive(Cw20ReceiveMsg),
    Distribute {
        distribution: Option<Distribution<String>>,
        /// Layered fees is an ordered list of fees to be applied before the distribution.
        /// The term layered refers to the implementation: Arena Tax -> Host Fee? -> Other Fee?
        /// Each fee is calculated based off the available funds at its layer
        layered_fees: Vec<FeeInformation<String>>,
        activation_height: Option<u64>,
        group_contract: String,
    },
    Lock {
        value: bool,
    },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(Vec<MemberAssets>)]
    Balances {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Vec<Assets>)]
    Balance { addr: String },
    #[returns(Vec<Assets>)]
    Due { addr: String },
    #[returns(Vec<MemberAssets>)]
    Dues {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Vec<MemberAssets>)]
    InitialDues {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(bool)]
    IsFunded { addr: String },
    #[returns(bool)]
    IsFullyFunded {},
    #[returns(Vec<Assets>)]
    TotalBalance {},
    #[returns(bool)]
    IsLocked {},
    #[returns(DumpStateResponse)]
    DumpState { addr: Option<String> },
}

#[cw_serde]
pub struct DumpStateResponse {
    pub is_locked: bool,
    pub total_balance: Vec<Assets>,
    pub balance: Vec<Assets>,
    pub due: Vec<Assets>,
}

#[cw_serde]
pub enum MigrateMsg {
    FromCompatible {},
}
