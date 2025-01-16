use arena_discord_identity::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use cw_orch::interface;
use cw_orch::prelude::*;

pub const CONTRACT_ID: &str = "arena_discord_identity";

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg, id = CONTRACT_ID)]
pub struct ArenaDiscordIdentityContract;

impl<Chain> Uploadable for ArenaDiscordIdentityContract<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path(CONTRACT_ID)
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(
            ContractWrapper::new_with_empty(
                arena_discord_identity::contract::execute,
                arena_discord_identity::contract::instantiate,
                arena_discord_identity::contract::query,
            )
            .with_migrate(arena_discord_identity::contract::migrate),
        )
    }
}
