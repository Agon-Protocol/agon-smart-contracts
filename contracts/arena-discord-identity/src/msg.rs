use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint64};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub faucet_amount: Coin,
}

#[cw_ownable::cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    SetProfile {
        addr: String,
        discord_profile: DiscordProfile,
    },
    SetConnections {
        connections: Vec<DiscordConnection>,
    },
    SetFaucetAmount {
        amount: Coin,
    },
    RemoveProfile {},
    Withdraw {
        funds: Vec<Coin>,
    },
}

#[cw_ownable::cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(Option<DiscordProfile>)]
    DiscordProfile { addr: String },
    #[returns(Vec<cosmwasm_std::Addr>)]
    ConnectedWallets { discord_id: Uint64 },
    #[returns(Vec<DiscordConnection>)]
    DiscordConnections { addr: String },
}

#[cw_serde]
pub enum MigrateMsg {
    FromCompatible {},
    SetHasReceived {},
}

#[cw_serde]
pub struct DiscordProfile {
    pub user_id: Uint64,
    /// The discord username
    pub username: String,
    pub avatar_hash: Option<String>,
}

#[cw_serde]
pub struct DiscordConnection {
    /// The type of service connection
    pub key: String,
    /// The service's connection username
    pub username: String,
}
