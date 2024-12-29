use cosmwasm_std::{Addr, Deps, Env, StdResult};
use cw_balance::Distribution;

use crate::state::PRESET_DISTRIBUTIONS;

pub fn get_distribution(
    deps: Deps,
    env: Env,
    addr: String,
    height: Option<u64>,
) -> StdResult<Option<Distribution<Addr>>> {
    let addr = deps.api.addr_validate(&addr)?;
    let height = height.unwrap_or(env.block.height);

    PRESET_DISTRIBUTIONS.may_load_at_height(deps.storage, &addr, height)
}

pub fn get_distributions(
    deps: Deps,
    env: Env,
    addrs: Vec<String>,
    height: Option<u64>,
) -> StdResult<Vec<(Addr, Distribution<Addr>)>> {
    let mut result = vec![];
    let height = height.unwrap_or(env.block.height);

    for addr in addrs {
        let addr = deps.api.addr_validate(&addr)?;

        if let Some(distribution) =
            PRESET_DISTRIBUTIONS.may_load_at_height(deps.storage, &addr, height)?
        {
            result.push((addr, distribution));
        }
    }

    Ok(result)
}
