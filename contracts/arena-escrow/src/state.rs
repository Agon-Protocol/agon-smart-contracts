use cosmwasm_std::{Addr, Deps, Uint128};
use cw_balance::Assets;
use cw_storage_plus::{Item, Map};

pub const EXISTING_BALANCES: Map<&Addr, ()> = Map::new("existing_balances");
pub const NATIVE_BALANCES: Map<(&Addr, &str), Uint128> = Map::new("native_balances");
pub const CW20_BALANCES: Map<(&Addr, &Addr), Uint128> = Map::new("cw20_balances");
pub const CW721_BALANCES: Map<(&Addr, &Addr), Vec<String>> = Map::new("cw721_balances");

pub const TOTAL_NATIVE_BALANCES: Map<&str, Uint128> = Map::new("total_native_balances");
pub const TOTAL_CW20_BALANCES: Map<&Addr, Uint128> = Map::new("total_cw20_balances");
pub const TOTAL_CW721_BALANCES: Map<&Addr, Vec<String>> = Map::new("total_cw721_balances");

pub const INITIAL_DUES: Map<&Addr, Vec<Assets>> = Map::new("initial_dues");
pub const DUES: Map<&Addr, Vec<Assets>> = Map::new("dues");

pub const IS_LOCKED: Item<bool> = Item::new("is_locked");
pub const HAS_DISTRIBUTED: Item<bool> = Item::new("has_distributed");

pub fn is_fully_funded(deps: Deps) -> bool {
    DUES.is_empty(deps.storage)
}

pub fn is_funded(deps: Deps, addr: &Addr) -> bool {
    !DUES.has(deps.storage, addr)
}
