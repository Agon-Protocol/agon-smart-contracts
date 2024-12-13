use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, WasmMsg,
};
use cw2::{ensure_from_older_version, set_contract_version};
use cw_ownable::assert_owner;

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{DISCORD_IDENTITY, FAUCET_AMOUNT, REVERSE_IDENTITY_MAP},
    ContractError,
};

pub(crate) const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;
    let ownership = cw_ownable::initialize_owner(deps.storage, deps.api, Some(owner.as_str()))?;

    Ok(Response::new()
        .add_attributes(ownership.into_attributes())
        .add_message(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_json_binary(&ExecuteMsg::SetFaucetAmount {
                amount: msg.faucet_amount,
            })?,
            funds: vec![],
        }))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        assert_owner(deps.storage, &info.sender)?;
    }

    match msg {
        ExecuteMsg::SetProfile { addr, user_id } => {
            let user = deps.api.addr_validate(&addr)?;
            let mut msgs = vec![];
            if !DISCORD_IDENTITY.has(deps.storage, &user)
                && !REVERSE_IDENTITY_MAP.has(deps.storage, user_id.u64())
            {
                let amount = vec![FAUCET_AMOUNT.load(deps.storage)?];
                msgs.push(BankMsg::Send {
                    to_address: user.to_string(),
                    amount,
                })
            }

            DISCORD_IDENTITY.save(deps.storage, &user, &user_id)?;
            REVERSE_IDENTITY_MAP.save(deps.storage, user_id.u64(), &user)?;

            Ok(Response::new().add_messages(msgs))
        }
        ExecuteMsg::SetFaucetAmount { amount } => {
            FAUCET_AMOUNT.save(deps.storage, &amount)?;

            Ok(Response::new())
        }
        ExecuteMsg::Withdraw {} => {
            let funds = deps
                .querier
                .query_all_balances(env.contract.address.to_string())?;

            Ok(Response::new().add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: funds,
            }))
        }
        ExecuteMsg::UpdateOwnership(action) => {
            let ownership = cw_ownable::update_ownership(deps, &env.block, &info.sender, action)?;
            Ok(Response::new().add_attributes(ownership.into_attributes()))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::UserId { addr } => {
            let addr = deps.api.addr_validate(&addr)?;

            to_json_binary(&DISCORD_IDENTITY.may_load(deps.storage, &addr)?)
        }
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    let _version = ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
