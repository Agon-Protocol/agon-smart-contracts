use arena_interface::escrow::DumpStateResponse;
use cosmwasm_std::{Addr, Coin, Deps, Order, StdResult};
use cw20::Cw20CoinVerified;
use cw_balance::{Assets, MemberAssets};
use cw_storage_plus::Bound;
use cw_utils::maybe_addr;

use crate::state::{
    CW20_BALANCES, DUES, EXISTING_BALANCES, INITIAL_DUES, IS_LOCKED, NATIVE_BALANCES,
    TOTAL_CW20_BALANCES, TOTAL_NATIVE_BALANCES,
};

pub fn balance(deps: Deps, addr: &Addr) -> StdResult<Vec<Assets>> {
    let mut assets = vec![];

    let coins = NATIVE_BALANCES
        .prefix(addr)
        .range(deps.storage, None, None, Order::Descending)
        .map(|x| {
            let (k, v) = x?;

            Ok(Coin {
                denom: k,
                amount: v,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    if !coins.is_empty() {
        assets.push(Assets::Native(coins));
    }

    let coins = CW20_BALANCES
        .prefix(addr)
        .range(deps.storage, None, None, Order::Descending)
        .map(|x| {
            let (k, v) = x?;

            Ok(Cw20CoinVerified {
                address: k,
                amount: v,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    if !coins.is_empty() {
        assets.push(Assets::Cw20(coins));
    }

    Ok(assets)
}

pub fn due(deps: Deps, addr: String) -> StdResult<Vec<Assets>> {
    let addr = deps.api.addr_validate(&addr)?;

    Ok(DUES.may_load(deps.storage, &addr)?.unwrap_or_default())
}

pub fn total_balance(deps: Deps) -> StdResult<Vec<Assets>> {
    let mut assets = vec![];

    let coins = TOTAL_NATIVE_BALANCES
        .range(deps.storage, None, None, Order::Descending)
        .map(|x| {
            let (k, v) = x?;

            Ok(Coin {
                denom: k,
                amount: v,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    if !coins.is_empty() {
        assets.push(Assets::Native(coins));
    }

    let coins = TOTAL_CW20_BALANCES
        .range(deps.storage, None, None, Order::Descending)
        .map(|x| {
            let (k, v) = x?;

            Ok(Cw20CoinVerified {
                address: k,
                amount: v,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    if !coins.is_empty() {
        assets.push(Assets::Cw20(coins));
    }

    Ok(assets)
}

pub fn is_locked(deps: Deps) -> bool {
    IS_LOCKED.load(deps.storage).unwrap_or_default()
}

pub fn is_funded(deps: Deps, addr: String) -> StdResult<bool> {
    let addr = deps.api.addr_validate(&addr)?;
    Ok(crate::state::is_funded(deps, &addr))
}

pub fn balances(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<MemberAssets>> {
    let binding = maybe_addr(deps.api, start_after)?;
    let start = binding.as_ref().map(Bound::exclusive);
    let mut balances = vec![];

    let targets = cw_paginate::paginate_map(
        &EXISTING_BALANCES,
        deps.storage,
        start,
        limit,
        |k, _v| -> StdResult<_> { Ok(k) },
    )?;

    for target in targets {
        let balance = balance(deps, &target)?;
        balances.push(MemberAssets {
            addr: target,
            assets: balance,
        });
    }

    Ok(balances)
}

pub fn dues(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<MemberAssets>> {
    let binding = maybe_addr(deps.api, start_after)?;
    let start = binding.as_ref().map(Bound::exclusive);
    cw_paginate::paginate_map(&DUES, deps.storage, start, limit, |k, v| {
        Ok(MemberAssets { addr: k, assets: v })
    })
}

pub fn initial_dues(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<MemberAssets>> {
    let binding = maybe_addr(deps.api, start_after)?;
    let start = binding.as_ref().map(Bound::exclusive);
    cw_paginate::paginate_map(&INITIAL_DUES, deps.storage, start, limit, |k, v| {
        Ok(MemberAssets { addr: k, assets: v })
    })
}

pub fn dump_state(deps: Deps, addr: Option<String>) -> StdResult<DumpStateResponse> {
    let maybe_addr = maybe_addr(deps.api, addr)?;
    let balance = maybe_addr
        .as_ref()
        .map(|x| balance(deps, x))
        .transpose()?
        .unwrap_or_default();
    let due = maybe_addr
        .map(|x| due(deps, x.to_string()))
        .transpose()?
        .unwrap_or_default();

    Ok(DumpStateResponse {
        due,
        is_locked: is_locked(deps),
        total_balance: total_balance(deps)?,
        balance,
    })
}
