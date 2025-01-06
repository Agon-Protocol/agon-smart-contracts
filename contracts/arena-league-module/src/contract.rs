use arena_interface::competition::msg::{ExecuteBase, MigrateBase, QueryBase};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure_eq, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Reply,
    Response, StdResult, WasmMsg,
};
use cw2::{ensure_from_older_version, set_contract_version};
use cw_competition_base::{contract::CompetitionModuleContract, error::CompetitionError};

use crate::{
    execute, migrate,
    msg::{
        ExecuteExt, ExecuteMsg, InstantiateMsg, LeagueInstantiateExt, LeagueQueryExt, MigrateMsg,
        QueryMsg,
    },
    query,
    state::LeagueExt,
    ContractError,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:arena-league-module";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub type CompetitionModule<'a> = CompetitionModuleContract<
    'a,
    Empty,
    ExecuteExt,
    LeagueQueryExt,
    LeagueExt,
    LeagueInstantiateExt,
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
            host,
            category_id,
            escrow,
            name,
            description,
            expiration,
            rules,
            rulesets,
            banner,
            instantiate_extension,
            group_contract,
        } => Ok(CompetitionModule::default()
            .execute_create_competition(
                &mut deps,
                &env,
                &info,
                host,
                category_id,
                escrow,
                name,
                description,
                expiration,
                rules,
                rulesets,
                banner,
                group_contract,
                instantiate_extension,
            )?
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::Extension {
                    msg: ExecuteExt::InstantiateRounds {},
                })?,
                funds: vec![],
            }))),
        ExecuteBase::Extension { msg } => match msg {
            ExecuteExt::ProcessMatch {
                league_id,
                round_number,
                match_results,
            } => execute::process_matches(deps, info, league_id, round_number, match_results),
            ExecuteExt::UpdateDistribution {
                league_id,
                distribution,
            } => execute::update_distribution(deps, info, league_id, distribution),
            ExecuteExt::AddPointAdjustments {
                league_id,
                addr,
                point_adjustments,
            } => execute::add_point_adjustments(deps, info, league_id, addr, point_adjustments),
            ExecuteExt::InstantiateRounds {} => execute::instantiate_rounds(deps, env, info),
        },
        ExecuteBase::ProcessCompetition {
            competition_id,
            distribution,
        } => {
            let competition = CompetitionModule::default()
                .competitions
                .load(deps.storage, competition_id.u128())?;
            ensure_eq!(
                info.sender.clone(),
                competition.admin_dao,
                ContractError::CompetitionError(CompetitionError::Unauthorized {})
            );

            Ok(CompetitionModule::default().execute_process_competition(
                deps,
                info,
                competition_id,
                distribution,
                None,
            )?)
        }
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
            LeagueQueryExt::Leaderboard { league_id, round } => {
                to_json_binary(&query::leaderboard(deps, league_id, round)?)
            }
            LeagueQueryExt::Round {
                league_id,
                round_number,
            } => to_json_binary(&query::round(deps, league_id, round_number)?),
            LeagueQueryExt::PointAdjustments {
                league_id,
                start_after,
                limit,
            } => to_json_binary(&query::point_adjustments(
                deps,
                league_id,
                start_after,
                limit,
            )?),
            LeagueQueryExt::DumpState {
                league_id,
                round_number,
            } => to_json_binary(&query::dump_state(deps, league_id, round_number)?),
        },
        _ => CompetitionModule::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(mut deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let competition_module = CompetitionModule::default();
    let version = ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut msgs = vec![];
    match msg {
        MigrateMsg::Base(migrate_base) => match migrate_base {
            MigrateBase::FromCompatible {} => {
                if version.major == 1 && version.minor == 3 {
                    migrate::from_v1_3_to_v_1_4(deps.branch())?;
                }
                if version.major == 1 && version.minor < 7 {
                    competition_module.migrate_from_v1_6_to_v1_7(deps.branch())?;
                }
            }
            MigrateBase::FromV2_2 { escrow_id } => {
                msgs.extend(competition_module.migrate_from_v2_2_to_v2_3(
                    deps.branch(),
                    env,
                    escrow_id,
                )?);
            }
        },
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default().add_messages(msgs))
}
