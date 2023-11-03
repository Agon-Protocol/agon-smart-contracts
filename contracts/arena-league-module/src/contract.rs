#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Reply, Response, StdResult,
};
use cw2::set_contract_version;
use cw_competition::msg::{ExecuteBase, QueryBase};
use cw_competition_base::{contract::CompetitionModuleContract, error::CompetitionError};

use crate::{
    execute,
    msg::{
        CompetitionExt, CompetitionInstantiateExt, ExecuteExt, ExecuteMsg, InstantiateMsg,
        MigrateMsg, QueryExt, QueryMsg,
    },
    query, ContractError,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:arena-league-module";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub type CompetitionModule = CompetitionModuleContract<
    Empty,
    ExecuteExt,
    QueryExt,
    CompetitionExt,
    CompetitionInstantiateExt,
>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let resp = CompetitionModule::default().instantiate(deps.branch(), env, info, msg)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteBase::CreateCompetition {
            competition_dao,
            escrow,
            name,
            description,
            expiration,
            rules,
            rulesets,
            instantiate_extension,
        } => {
            let response = CompetitionModule::default().execute_create_competition(
                &mut deps,
                &env,
                competition_dao,
                escrow,
                name,
                description,
                expiration,
                rules,
                rulesets,
                instantiate_extension.clone(),
            )?;

            execute::instantiate_rounds(
                deps,
                env,
                response,
                instantiate_extension.teams,
                instantiate_extension.round_duration,
            )
        }
        ExecuteBase::Extension { msg } => match msg {
            ExecuteExt::ProcessMatch {
                league_id,
                round_number,
                match_number,
                result,
            } => execute::process_match(
                deps,
                env,
                info,
                league_id,
                round_number,
                match_number,
                result,
            ),
        },
        _ => Ok(CompetitionModule::default().execute(deps, env, info, msg)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, CompetitionError> {
    CompetitionModule::default().reply(deps, env, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryBase::QueryExtension { msg } => match msg {
            QueryExt::Leaderboard { league_id } => to_binary(&query::leaderboard(deps, league_id)?),
            QueryExt::Round {
                league_id,
                round_number,
            } => to_binary(&query::round(deps, league_id, round_number)?),
        },
        _ => CompetitionModule::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, CompetitionError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
