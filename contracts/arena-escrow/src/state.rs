use cosmwasm_std::{Addr, Deps};
use cw_balance::BalanceVerified;
use cw_storage_plus::{Item, Map};

pub const ENROLLMENT_CONTRACT: Item<Addr> = Item::new("enrollment_contract");
pub const TOTAL_BALANCE: Item<BalanceVerified> = Item::new("total");
pub const BALANCE: Map<&Addr, BalanceVerified> = Map::new("balance");
pub const INITIAL_DUE: Map<&Addr, BalanceVerified> = Map::new("initial_due");
pub const DUE: Map<&Addr, BalanceVerified> = Map::new("due");
pub const IS_LOCKED: Item<bool> = Item::new("is_locked");
pub const HAS_DISTRIBUTED: Item<bool> = Item::new("has_distributed");

pub fn is_fully_funded(deps: Deps) -> bool {
    DUE.is_empty(deps.storage)
}

pub fn is_funded(deps: Deps, addr: &Addr) -> bool {
    !DUE.has(deps.storage, addr)
}
