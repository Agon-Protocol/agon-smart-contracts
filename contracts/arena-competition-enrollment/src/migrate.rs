use arena_interface::escrow;
use cosmwasm_std::{
    instantiate2_address, to_json_binary, CosmosMsg, DepsMut, Env, Order, StdResult, WasmMsg,
};
use sha2::{Digest as _, Sha256};

use crate::{
    state::{
        enrollment_entries, CompetitionInfo, EnrollmentEntry, LegacyCompetitionInfo,
        LEGACY_ENROLLMENTS,
    },
    ContractError,
};

pub fn migrate_from_v2_2_to_v2_3(
    deps: DepsMut,
    env: Env,
    escrow_id: u64,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut msgs = vec![];
    let code_info = deps.querier.query_wasm_code_info(escrow_id)?;

    for (enrollment_id, enrollment) in LEGACY_ENROLLMENTS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?
    {
        let competition_info = match enrollment.competition_info {
            LegacyCompetitionInfo::Pending {
                name,
                description,
                expiration,
                rules,
                rulesets,
                banner,
                additional_layered_fees,
            } => {
                let binding = format!("{}{}{}", "enrollments", env.block.height, enrollment_id);
                let salt: [u8; 32] = Sha256::digest(binding.as_bytes()).into();
                let canonical_creator =
                    deps.api.addr_canonicalize(env.contract.address.as_str())?;
                let canonical_addr =
                    instantiate2_address(&code_info.checksum, &canonical_creator, &salt)?;

                msgs.push(CosmosMsg::Wasm(WasmMsg::Instantiate2 {
                    admin: Some(env.contract.address.to_string()),
                    code_id: escrow_id,
                    label: "Arena Escrow".to_string(),
                    msg: to_json_binary(&escrow::InstantiateMsg {
                        dues: vec![],
                        is_enrollment: true,
                    })?,
                    funds: vec![],
                    salt: salt.into(),
                }));

                let escrow = deps.api.addr_humanize(&canonical_addr)?;

                CompetitionInfo::Pending {
                    name,
                    description,
                    expiration,
                    rules,
                    rulesets,
                    banner,
                    additional_layered_fees,
                    escrow,
                    group_contract: enrollment.group_contract,
                }
            }
            LegacyCompetitionInfo::Existing { id } => CompetitionInfo::Existing { id },
        };

        let new_enrollment = EnrollmentEntry {
            min_members: enrollment.min_members,
            max_members: enrollment.max_members,
            entry_fee: enrollment.entry_fee,
            expiration: enrollment.expiration,
            has_finalized: enrollment.has_triggered_expiration,
            competition_info,
            competition_type: enrollment.competition_type,
            host: enrollment.host,
            category_id: enrollment.category_id,
            competition_module: enrollment.competition_module,
            required_team_size: enrollment.required_team_size,
        };

        enrollment_entries().replace(
            deps.storage,
            enrollment_id,
            Some(&new_enrollment),
            Some(&new_enrollment),
        )?;
    }

    Ok(msgs)
}
