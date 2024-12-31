use arena_interface::{
    competition::msg::EscrowContractInfo,
    core::{CompetitionModuleQuery, CompetitionModuleResponse},
    escrow::{self, EnrollmentWithdrawMsg},
    fees::FeeInformation,
    group::{self, GroupContractInfo, MemberMsg},
};
use arena_league_module::msg::LeagueInstantiateExt;
use arena_tournament_module::{msg::TournamentInstantiateExt, state::EliminationType};
use arena_wager_module::msg::WagerInstantiateExt;
use cosmwasm_std::{
    ensure, instantiate2_address, to_json_binary, Addr, Attribute, Coin, CosmosMsg, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, SubMsg, Uint128, Uint64, WasmMsg,
};
use cw_utils::{must_pay, Expiration};
use dao_interface::{state::ModuleInstantiateInfo, voting::VotingPowerAtHeightResponse};
use itertools::Itertools as _;
use sha2::{Digest, Sha256};

use crate::{
    msg::CompetitionInfoMsg,
    state::{
        enrollment_entries, CompetitionInfo, CompetitionType, EnrollmentEntry, EnrollmentInfo,
        ENROLLMENT_COUNT, TEMP_ENROLLMENT_INFO,
    },
    ContractError,
};

pub const FINALIZE_COMPETITION_REPLY_ID: u64 = 1;
/// Team size is limited to cw4-group's max size limit
const TEAM_SIZE_LIMIT: u32 = 30;

#[allow(clippy::too_many_arguments)]
pub fn create_enrollment(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    min_members: Option<Uint64>,
    max_members: Uint64,
    entry_fee: Option<Coin>,
    expiration: Expiration,
    category_id: Option<Uint128>,
    competition_info: CompetitionInfoMsg,
    competition_type: CompetitionType,
    group_contract_info: ModuleInstantiateInfo,
    required_team_size: Option<u32>,
    escrow_contract_info: EscrowContractInfo,
) -> Result<Response, ContractError> {
    ensure!(
        !expiration.is_expired(&env.block),
        ContractError::StdError(StdError::generic_err(
            "Cannot create an expired competition enrollment"
        ))
    );
    ensure!(
        expiration < competition_info.expiration,
        ContractError::StdError(StdError::generic_err(
            "Cannot have an enrollment with expiration before the competition's expiration"
        ))
    );

    let min_min_members = get_min_min_members(&competition_type);
    if let Some(min_members) = min_members {
        ensure!(
            min_members <= max_members,
            ContractError::StdError(StdError::generic_err(
                "Min members cannot be larger than max members"
            ))
        );
        ensure!(
            min_members >= min_min_members,
            ContractError::StdError(StdError::generic_err(format!(
                "Min members cannot be less than the required minimum of {}",
                min_min_members
            )))
        )
    } else {
        ensure!(
            min_min_members <= max_members,
            ContractError::StdError(StdError::generic_err(
                "Max members must be at least the required minimum number of members"
            ))
        );
    }

    // Validate category
    let ownership = cw_ownable::get_ownership(deps.storage)?;
    let competition_module = if let Some(owner) = ownership.owner {
        if let Some(category_id) = category_id {
            if let Some(rulesets) = &competition_info.rulesets {
                ensure!(
                    deps.querier.query_wasm_smart::<bool>(
                        &owner,
                        &arena_interface::core::QueryMsg::QueryExtension {
                            msg: arena_interface::core::QueryExt::IsValidCategoryAndRulesets {
                                category_id,
                                rulesets: rulesets.clone(),
                            },
                        },
                    )?,
                    ContractError::StdError(StdError::generic_err(
                        "Invalid category and rulesets combination"
                    ))
                );
            }
        }

        let competition_module_response = deps
            .querier
            .query_wasm_smart::<Option<CompetitionModuleResponse<Addr>>>(
                owner,
                &arena_interface::core::QueryMsg::QueryExtension {
                    msg: arena_interface::core::QueryExt::CompetitionModule {
                        query: CompetitionModuleQuery::Key(competition_type.to_string(), None),
                    },
                },
            )?;

        if let Some(competition_module) = competition_module_response {
            ensure!(
                competition_module.is_enabled,
                ContractError::StdError(StdError::generic_err(
                    "Cannot use a disabled competition module"
                ))
            );

            Ok(competition_module.addr)
        } else {
            Err(ContractError::StdError(StdError::generic_err(
                "Could not find the competition module",
            )))
        }
    } else {
        Err(ContractError::OwnershipError(
            cw_ownable::OwnershipError::NoOwner,
        ))
    }?;

    let competition_id = ENROLLMENT_COUNT.update(deps.storage, |x| -> StdResult<_> {
        Ok(x.checked_add(Uint128::one())?)
    })?;

    let mut msgs = vec![];

    // Generate the group contract
    let binding = format!("{}{}{}", info.sender, env.block.height, competition_id);
    let salt: [u8; 32] = Sha256::digest(binding.as_bytes()).into();
    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let code_info = deps
        .querier
        .query_wasm_code_info(group_contract_info.code_id)?;
    let canonical_addr = instantiate2_address(&code_info.checksum, &canonical_creator, &salt)?;

    msgs.push(CosmosMsg::Wasm(WasmMsg::Instantiate2 {
        admin: Some(env.contract.address.to_string()),
        code_id: group_contract_info.code_id,
        label: group_contract_info.label,
        msg: group_contract_info.msg,
        funds: vec![],
        salt: salt.into(),
    }));

    let group_contract = deps.api.addr_humanize(&canonical_addr)?;

    // Handle escrow setup
    let (escrow_addr, fees) = match escrow_contract_info {
        EscrowContractInfo::Existing {
            addr,
            additional_layered_fees,
        } => {
            let fees = additional_layered_fees
                .map(|fees| {
                    fees.iter()
                        .map(|fee| fee.into_checked(deps.as_ref()))
                        .collect::<StdResult<Vec<_>>>()
                })
                .transpose()?;

            (addr, fees)
        }
        EscrowContractInfo::New {
            code_id,
            msg,
            label,
            additional_layered_fees,
        } => {
            let fees = additional_layered_fees
                .map(|fees| {
                    fees.iter()
                        .map(|fee| fee.into_checked(deps.as_ref()))
                        .collect::<StdResult<Vec<_>>>()
                })
                .transpose()?;

            let binding = format!("{}{}{}", info.sender, env.block.height, competition_id);
            let salt: [u8; 32] = Sha256::digest(binding.as_bytes()).into();
            let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
            let code_info = deps.querier.query_wasm_code_info(code_id)?;
            let canonical_addr =
                instantiate2_address(&code_info.checksum, &canonical_creator, &salt)?;

            msgs.push(CosmosMsg::Wasm(WasmMsg::Instantiate2 {
                admin: Some(env.contract.address.to_string()),
                code_id,
                label,
                msg,
                funds: vec![],
                salt: salt.into(),
            }));

            let escrow_addr = deps.api.addr_humanize(&canonical_addr)?;

            (escrow_addr, fees)
        }
    };

    enrollment_entries().save(
        deps.storage,
        competition_id.u128(),
        &EnrollmentEntry {
            min_members,
            max_members,
            entry_fee,
            expiration,
            has_finalized: false,
            competition_info: CompetitionInfo::Pending {
                name: competition_info.name,
                description: competition_info.description,
                expiration: competition_info.expiration,
                rules: competition_info.rules,
                rulesets: competition_info.rulesets,
                banner: competition_info.banner,
                additional_layered_fees: fees,
            },
            competition_type,
            host: info.sender,
            category_id,
            competition_module,
            group_contract,
            required_team_size,
            escrow: escrow_addr,
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "create_enrollment")
        .add_attribute("id", competition_id)
        .add_messages(msgs))
}

pub fn finalize(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: Uint128,
) -> Result<Response, ContractError> {
    let entry = enrollment_entries().load(deps.storage, id.u128())?;

    ensure!(entry.host == info.sender, ContractError::Unauthorized {});
    ensure!(!entry.has_finalized, ContractError::AlreadyFinalized {});

    let members_count: Uint64 = deps.querier.query_wasm_smart(
        entry.group_contract.to_string(),
        &group::QueryMsg::MembersCount {},
    )?;

    // Check if we have met the minimum number of members
    let min_min_members = get_min_min_members(&entry.competition_type);
    let min_members = entry.min_members.unwrap_or(min_min_members);
    let is_expired = entry.expiration.is_expired(&env.block);
    let new_data = EnrollmentEntry {
        has_finalized: true,
        ..entry.clone()
    };

    if members_count < min_members && is_expired {
        enrollment_entries().replace(deps.storage, id.u128(), Some(&new_data), Some(&entry))?;

        // Return a response indicating the enrollment was expired due to insufficient members
        return Ok(Response::new()
            .add_attribute("action", "finalize")
            .add_attribute("result", "finalized_insufficient_members")
            .add_attribute("id", id.to_string())
            .add_attribute("required_members", min_members.to_string())
            .add_attribute("actual_members", members_count.to_string()));
    }

    ensure!(
        entry.max_members == members_count || is_expired,
        ContractError::FinalizeFailed {
            max_members: entry.max_members,
            current_members: members_count,
            expiration: entry.expiration
        }
    );

    let enrollment_info = EnrollmentInfo {
        enrollment_id: id.u128(),
        module_addr: entry.competition_module.clone(),
        escrow_addr: entry.escrow.clone(),
    };

    let creation_msg = match entry.competition_info.clone() {
        CompetitionInfo::Pending {
            name,
            description,
            expiration,
            rules,
            rulesets,
            banner,
            additional_layered_fees,
        } => Ok({
            let additional_layered_fees = additional_layered_fees.map(|x| {
                x.into_iter()
                    .map(|y| FeeInformation {
                        tax: y.tax,
                        receiver: y.receiver.to_string(),
                        cw20_msg: y.cw20_msg,
                        cw721_msg: y.cw721_msg,
                    })
                    .collect_vec()
            });

            let escrow = EscrowContractInfo::Existing {
                addr: entry.escrow.clone(),
                additional_layered_fees,
            };

            match entry.competition_type.clone() {
                CompetitionType::Wager {} => {
                    to_json_binary(&arena_wager_module::msg::ExecuteMsg::CreateCompetition {
                        host: Some(entry.host.to_string()),
                        category_id: entry.category_id,
                        escrow,
                        name,
                        description,
                        expiration,
                        rules,
                        rulesets,
                        banner,
                        instantiate_extension: WagerInstantiateExt {},
                        group_contract: GroupContractInfo::Existing {
                            addr: entry.group_contract.to_string(),
                        },
                    })?
                }
                CompetitionType::League {
                    match_win_points,
                    match_draw_points,
                    match_lose_points,
                    distribution,
                } => to_json_binary(&arena_league_module::msg::ExecuteMsg::CreateCompetition {
                    host: Some(entry.host.to_string()),
                    category_id: entry.category_id,
                    escrow,
                    name,
                    description,
                    expiration,
                    rules,
                    rulesets,
                    banner,
                    instantiate_extension: LeagueInstantiateExt {
                        match_win_points,
                        match_draw_points,
                        match_lose_points,
                        distribution,
                    },
                    group_contract: GroupContractInfo::Existing {
                        addr: entry.group_contract.to_string(),
                    },
                })?,
                CompetitionType::Tournament {
                    elimination_type,
                    distribution,
                } => to_json_binary(
                    &arena_tournament_module::msg::ExecuteMsg::CreateCompetition {
                        host: Some(entry.host.to_string()),
                        category_id: entry.category_id,
                        escrow,
                        name,
                        description,
                        expiration,
                        rules,
                        rulesets,
                        banner,
                        instantiate_extension: TournamentInstantiateExt {
                            elimination_type,
                            distribution,
                        },
                        group_contract: GroupContractInfo::Existing {
                            addr: entry.group_contract.to_string(),
                        },
                    },
                )?,
            }
        }),
        _ => Err(ContractError::AlreadyFinalized {}),
    }?;

    let sub_msg = SubMsg::reply_always(
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: entry.competition_module.to_string(),
            msg: creation_msg,
            funds: vec![],
        }),
        FINALIZE_COMPETITION_REPLY_ID,
    );

    enrollment_entries().replace(deps.storage, id.u128(), Some(&new_data), Some(&entry))?;
    TEMP_ENROLLMENT_INFO.save(deps.storage, &enrollment_info)?;

    Ok(Response::new()
        .add_attribute("action", "finalize")
        .add_attribute("competition_module", enrollment_info.module_addr)
        .add_attribute("id", id.to_string())
        .add_submessage(sub_msg))
}

pub fn enroll(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    id: Uint128,
    team: Option<String>,
) -> Result<Response, ContractError> {
    let entry = enrollment_entries().load(deps.storage, id.u128())?;

    ensure!(!entry.has_finalized, ContractError::AlreadyFinalized {});

    let mut msgs = vec![];
    if let Some(entry_fee) = entry.entry_fee {
        let paid_amount = must_pay(&info, &entry_fee.denom)?;

        ensure!(
            paid_amount == entry_fee.amount,
            ContractError::EntryFeeNotPaid { entry_fee }
        );

        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: entry.escrow.to_string(),
            msg: to_json_binary(&escrow::ExecuteMsg::ReceiveNative {})?,
            funds: vec![entry_fee],
        }));
    };

    let member_count: Uint64 = deps.querier.query_wasm_smart(
        entry.group_contract.to_string(),
        &group::QueryMsg::MembersCount {},
    )?;

    ensure!(
        member_count < entry.max_members,
        ContractError::EnrollmentMaxMembers {}
    );

    // Set correct member
    let member = if let Some(team) = team {
        let team = deps.api.addr_validate(&team)?;
        let voting_power_response: VotingPowerAtHeightResponse = deps.querier.query_wasm_smart(
            team.to_string(),
            &dao_interface::msg::QueryMsg::VotingPowerAtHeight {
                address: info.sender.to_string(),
                height: None,
            },
        )?;

        if voting_power_response.power.is_zero() {
            return Err(ContractError::NotTeamMember {});
        }

        team
    } else {
        info.sender
    };

    // Ensure team size requirement is handled
    if let Some(required_team_size) = entry.required_team_size {
        if required_team_size != 1 || deps.querier.query_wasm_contract_info(&member).is_ok() {
            let dao_voting_module: Addr = deps.querier.query_wasm_smart(
                member.to_string(),
                &dao_interface::msg::QueryMsg::VotingModule {},
            )?;
            let group_contract: Addr = deps.querier.query_wasm_smart(
                dao_voting_module,
                &dao_voting_cw4::msg::QueryMsg::GroupContract {},
            )?;
            let member_list_response: cw4::MemberListResponse = deps.querier.query_wasm_smart(
                group_contract,
                &cw4::Cw4QueryMsg::ListMembers {
                    start_after: None,
                    limit: Some(TEAM_SIZE_LIMIT),
                },
            )?;

            ensure!(
                member_list_response.members.len() as u32 == required_team_size,
                ContractError::TeamSizeMismatch { required_team_size }
            );
        }
    }

    msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: entry.group_contract.to_string(),
        msg: to_json_binary(&group::ExecuteMsg::UpdateMembers {
            to_add: Some(vec![group::AddMemberMsg {
                addr: member.to_string(),
                seed: None,
            }]),
            to_remove: None,
            to_update: None,
        })?,
        funds: vec![],
    }));

    Ok(Response::new()
        .add_attribute("action", "enroll")
        .add_messages(msgs))
}

pub fn withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    id: Uint128,
) -> Result<Response, ContractError> {
    // Load the enrollment entry
    let entry = enrollment_entries().load(deps.storage, id.u128())?;

    Ok(_withdraw(entry, vec![info.sender.to_string()], id)?.add_attribute("action", "withdraw"))
}

pub fn force_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    id: Uint128,
    members: Vec<String>,
) -> Result<Response, ContractError> {
    // Load the enrollment entry
    let entry = enrollment_entries().load(deps.storage, id.u128())?;

    ensure!(entry.host == info.sender, ContractError::Unauthorized {});

    let members = members.into_iter().unique().collect::<Vec<_>>();

    ensure!(
        !members.is_empty(),
        ContractError::StdError(StdError::generic_err(
            "No members to force_withdraw provided"
        ))
    );

    Ok(_withdraw(entry, members, id)?.add_attribute("action", "force_withdraw"))
}

pub fn _withdraw(
    entry: EnrollmentEntry,
    members: Vec<String>,
    id: Uint128,
) -> Result<Response, ContractError> {
    // If finalized and created, then we cannot withdraw through here anymore
    ensure!(
        !entry.has_finalized || matches!(entry.competition_info, CompetitionInfo::Pending { .. }),
        ContractError::AlreadyFinalized {}
    );

    // If there's an entry fee, create refund messages for each member
    let refund_msgs = if let Some(entry_fee) = &entry.entry_fee {
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: entry.escrow.to_string(),
            msg: to_json_binary(&escrow::ExecuteMsg::Withdraw {
                cw20_msg: None,
                cw721_msg: None,
                enrollment_withdraw_info: Some(EnrollmentWithdrawMsg {
                    addrs: members.clone(),
                    entry_fee: entry_fee.clone(),
                }),
            })?,
            funds: vec![],
        })]
    } else {
        vec![]
    };

    // Create group update message to remove all members
    let group_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: entry.group_contract.to_string(),
        msg: to_json_binary(&group::ExecuteMsg::UpdateMembers {
            to_add: None,
            to_update: None,
            to_remove: Some(members.clone()),
        })?,
        funds: vec![],
    });

    // Create attributes for each withdrawn member
    let member_attributes = members
        .into_iter()
        .map(|member| Attribute {
            key: "withdrawn_member".to_string(),
            value: member.to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Response::new()
        .add_message(group_msg)
        .add_messages(refund_msgs)
        .add_attribute("id", id.to_string())
        .add_attributes(member_attributes))
}

fn get_min_min_members(competition_type: &CompetitionType) -> Uint64 {
    match competition_type {
        CompetitionType::Wager {} => Uint64::new(2),
        CompetitionType::League { distribution, .. } => {
            Uint64::new(std::cmp::max(distribution.len(), 2) as u64)
        }
        CompetitionType::Tournament {
            elimination_type,
            distribution,
        } => match elimination_type {
            EliminationType::SingleElimination {
                play_third_place_match,
            } => Uint64::new(std::cmp::max(
                if *play_third_place_match { 4 } else { 3 },
                distribution.len(),
            ) as u64),
            EliminationType::DoubleElimination => {
                Uint64::new(std::cmp::max(3, distribution.len()) as u64)
            }
        },
    }
}

pub fn set_rankings(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    id: Uint128,
    rankings: Vec<MemberMsg<String>>,
) -> Result<Response, ContractError> {
    let enrollment = enrollment_entries().load(deps.storage, id.u128())?;

    ensure!(
        enrollment.host == info.sender,
        ContractError::Unauthorized {}
    );

    let msg = WasmMsg::Execute {
        contract_addr: enrollment.group_contract.to_string(),
        msg: to_json_binary(&group::ExecuteMsg::UpdateMembers {
            to_add: None,
            to_update: Some(rankings),
            to_remove: None,
        })?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_attribute("action", "set_rankings")
        .add_message(msg))
}
