use std::{collections::HashMap, iter};

use arena_interface::{fees::FeeInformation, group};
use cosmwasm_std::{
    ensure, to_json_binary, Addr, Binary, Coin, CosmosMsg, DepsMut, Empty, Env, MessageInfo, Order,
    Response, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20CoinVerified, Cw20ReceiveMsg};
use cw_balance::{Assets, Distribution, MemberAssetsUnchecked};
use cw_ownable::{assert_owner, get_ownership};

use crate::{
    query::{self, is_locked},
    state::{
        is_fully_funded, CW20_BALANCES, DUES, HAS_DISTRIBUTED, INITIAL_DUES, IS_LOCKED,
        NATIVE_BALANCES, TOTAL_CW20_BALANCES, TOTAL_NATIVE_BALANCES,
    },
    ContractError,
};

pub fn set_dues(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dues: Vec<MemberAssetsUnchecked>,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        assert_owner(deps.storage, &info.sender)?;
    }

    IS_LOCKED.save(deps.storage, &false)?;
    for member_asset in dues {
        let member_asset = member_asset.into_checked(deps.as_ref())?;

        if INITIAL_DUES.has(deps.storage, &member_asset.addr) {
            return Err(ContractError::DuplicateDues {
                address: member_asset.addr,
            });
        }

        INITIAL_DUES.save(deps.storage, &member_asset.addr, &member_asset.assets)?;
        DUES.save(deps.storage, &member_asset.addr, &member_asset.assets)?;
    }

    Ok(Response::new().add_attribute("action", "set_dues"))
}

pub fn withdraw(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Option<Binary>,
    cw721_msg: Option<Binary>,
) -> Result<Response, ContractError> {
    if is_locked(deps.as_ref()) {
        return Err(ContractError::Locked {});
    }

    let balance = query::balance(deps.as_ref(), &info.sender)?;

    ensure!(!balance.is_empty(), ContractError::NothingToWithdraw {});

    let msgs = balance
        .into_iter()
        .map(|x| {
            x.transmit_all(
                deps.as_ref(),
                &info.sender,
                cw20_msg.clone(),
                cw721_msg.clone(),
            )
        })
        .collect::<StdResult<Vec<_>>>()?
        .concat();

    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_attribute("addr", info.sender)
        .add_messages(msgs))
}

pub fn receive_native(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut msgs = vec![];
    let sender = info.sender;

    // Process each native coin received
    for coin in info.funds.iter() {
        // Update individual balance first
        NATIVE_BALANCES.update(
            deps.storage,
            (&sender, &coin.denom),
            |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_add(coin.amount)?) },
        )?;

        // Update total balance
        TOTAL_NATIVE_BALANCES.update(deps.storage, &coin.denom, |total| -> StdResult<_> {
            Ok(total.unwrap_or_default().checked_add(coin.amount)?)
        })?;

        // Process dues if they exist
        if let Some(mut vec) = DUES.may_load(deps.storage, &sender)? {
            let mut modified = false;
            let mut assets_to_remove = None;

            // Update dues for native tokens
            for (idx, assets) in vec.iter_mut().enumerate() {
                if let Assets::Native(coins) = assets {
                    // Find and update matching coin
                    if let Some(coin_idx) = coins.iter().position(|c| c.denom == coin.denom) {
                        modified = true;

                        // Deduct the amount from dues
                        let due_coin = &mut coins[coin_idx];
                        let reduction = due_coin.amount.min(coin.amount);
                        due_coin.amount -= reduction;

                        // Remove zero-amount coin
                        if due_coin.amount.is_zero() {
                            coins.remove(coin_idx);

                            // Mark assets for removal if empty
                            if coins.is_empty() {
                                assets_to_remove = Some(idx);
                            }
                        }
                        break; // Exit after finding and processing the matching coin
                    }
                }
            }

            // Only update storage if modifications were made
            if modified {
                // Remove empty assets if needed
                if let Some(idx) = assets_to_remove {
                    vec.remove(idx);
                }

                // Update or remove dues entry
                if vec.is_empty() {
                    DUES.remove(deps.storage, &sender);

                    if is_fully_funded(deps.as_ref()) {
                        IS_LOCKED.save(deps.storage, &true)?;

                        if let Some(owner) = get_ownership(deps.storage)?.owner {
                            msgs.push(WasmMsg::Execute {
                                contract_addr: owner.to_string(),
                                msg: to_json_binary(
                                    &arena_interface::competition::msg::ExecuteBase::ActivateCompetition::<
                                        Empty,
                                        Empty,
                                    > {},
                                )?,
                                funds: vec![],
                            });
                        }
                    }
                } else {
                    DUES.save(deps.storage, &sender, &vec)?;
                }
            }
        }
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "receive_native")
        .add_attribute("sender", sender)
        .add_attributes(
            info.funds
                .into_iter()
                .map(|c| vec![("denom", c.denom), ("amount", c.amount.to_string())])
                .flatten(),
        ))
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let Cw20ReceiveMsg { sender, amount, .. } = cw20_receive_msg;
    let sender = deps.api.addr_validate(&sender)?;

    // Update individual balance first
    CW20_BALANCES.update(
        deps.storage,
        (&sender, &info.sender),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_add(amount)?) },
    )?;

    // Update total balance
    TOTAL_CW20_BALANCES.update(deps.storage, &info.sender, |total| -> StdResult<_> {
        Ok(total.unwrap_or_default().checked_add(amount)?)
    })?;

    // Process dues if they exist
    let mut msgs = vec![];
    if let Some(mut vec) = DUES.may_load(deps.storage, &sender)? {
        let mut modified = false;
        let mut assets_to_remove = None;

        // Update dues for CW20 tokens
        for (idx, assets) in vec.iter_mut().enumerate() {
            if let Assets::Cw20(coins) = assets {
                // Find and update matching coin
                if let Some(coin_idx) = coins.iter().position(|c| c.address == info.sender) {
                    modified = true;

                    // Deduct the amount from dues
                    let coin = &mut coins[coin_idx];
                    let reduction = coin.amount.min(amount);
                    coin.amount -= reduction;

                    // Remove zero-amount coin
                    if coin.amount.is_zero() {
                        coins.remove(coin_idx);

                        // Mark assets for removal if empty
                        if coins.is_empty() {
                            assets_to_remove = Some(idx);
                        }
                    }
                    break; // Exit after finding and processing the matching coin
                }
            }
        }

        // Only update storage if modifications were made
        if modified {
            // Remove empty assets if needed
            if let Some(idx) = assets_to_remove {
                vec.remove(idx);
            }

            // Update or remove dues entry
            if vec.is_empty() {
                DUES.remove(deps.storage, &sender);

                if is_fully_funded(deps.as_ref()) {
                    IS_LOCKED.save(deps.storage, &true)?;

                    if let Some(owner) = get_ownership(deps.storage)?.owner {
                        msgs.push(WasmMsg::Execute {
                        contract_addr: owner.to_string(),
                        msg: to_json_binary(
                            &arena_interface::competition::msg::ExecuteBase::ActivateCompetition::<
                                Empty,
                                Empty,
                            > {},
                        )?,
                        funds: vec![],
                    });
                    }
                }
            } else {
                DUES.save(deps.storage, &sender, &vec)?;
            }
        }
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "receive_cw20")
        .add_attribute("sender", sender)
        .add_attribute("token", info.sender)
        .add_attribute("amount", amount))
}

pub fn distribute(
    deps: DepsMut,
    info: MessageInfo,
    fees: Vec<FeeInformation<String>>,
    distribution: Option<Distribution<String>>,
    activation_height: Option<u64>,
    group_contract: String,
) -> Result<Response, ContractError> {
    // Ensure the sender is the owner
    assert_owner(deps.storage, &info.sender)?;

    // Validate the group contract
    let group_contract = deps.api.addr_validate(&group_contract)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut total_native = TOTAL_NATIVE_BALANCES
        .range(deps.storage, None, None, Order::Descending)
        .collect::<StdResult<HashMap<_, _>>>()?;
    let mut total_cw20 = TOTAL_CW20_BALANCES
        .range(deps.storage, None, None, Order::Descending)
        .collect::<StdResult<HashMap<_, _>>>()?;
    let mut native_distributions: HashMap<&Addr, HashMap<String, Uint128>> = HashMap::new();
    let mut cw20_distributions: HashMap<&Addr, HashMap<Addr, Uint128>> = HashMap::new();
    let fees = fees
        .into_iter()
        .map(|x| x.into_checked(deps.as_ref()))
        .collect::<StdResult<Vec<_>>>()?;
    let distribution = distribution
        .map(|x| x.into_checked(deps.as_ref()))
        .transpose()?;
    let mut preset_distributions: HashMap<Addr, Distribution<Addr>> = HashMap::new();

    // Calculate fees
    for (denom, amount) in total_native.iter_mut() {
        for fee in fees.iter() {
            let fee_amount = amount.checked_mul_floor(fee.tax)?;

            if !fee_amount.is_zero() {
                native_distributions
                    .entry(&fee.receiver)
                    .or_default()
                    .entry(denom.clone())
                    .and_modify(|e| *e += fee_amount)
                    .or_insert(fee_amount);

                *amount = amount.checked_sub(fee_amount)?;
            }
        }

        TOTAL_NATIVE_BALANCES.save(deps.storage, denom, amount)?;
    }

    for (addr, amount) in total_cw20.iter_mut() {
        for fee in fees.iter() {
            let fee_amount = amount.checked_mul_floor(fee.tax)?;

            if !fee_amount.is_zero() {
                cw20_distributions
                    .entry(&fee.receiver)
                    .or_default()
                    .entry(addr.clone())
                    .and_modify(|e| *e += fee_amount)
                    .or_insert(fee_amount);

                *amount = amount.checked_sub(fee_amount)?;
            }
        }

        TOTAL_CW20_BALANCES.save(deps.storage, addr, amount)?;
    }

    // Calculate and track member distributions for native tokens
    if let Some(distribution) = distribution.as_ref() {
        // Validate distribution is valid
        if !deps.querier.query_wasm_smart::<bool>(
            group_contract.to_string(),
            &group::QueryMsg::IsValidDistribution {
                addrs: distribution
                    .member_percentages
                    .iter()
                    .map(|x| x.addr.to_string())
                    .chain(iter::once(distribution.remainder_addr.to_string()))
                    .collect(),
            },
        )? {
            return Err(ContractError::InvalidDistribution {
                msg: "The distribution must contain only members of the competition".to_string(),
            });
        }

        // Query payment registry from the competition contract (sender)
        let payment_registry: Option<String> =
            deps.querier.query_wasm_smart(
                info.sender.to_string(),
                &arena_interface::competition::msg::QueryBase::PaymentRegistry::<
                    Empty,
                    Empty,
                    Empty,
                > {},
            )?;

        if let Some(payment_registry) = payment_registry {
            let payment_registry = deps.api.addr_validate(&payment_registry)?;

            preset_distributions = deps.querier.query_wasm_smart(
                payment_registry,
                &arena_interface::registry::QueryMsg::GetDistributions {
                    addrs: distribution
                        .member_percentages
                        .iter()
                        .map(|x| x.addr.to_string())
                        .collect(),
                    height: activation_height,
                },
            )?;
        }

        for (denom, amount) in total_native.into_iter() {
            if amount.is_zero() {
                continue;
            }

            let mut remaining_amount = amount;

            // Member percentages
            for member in distribution.member_percentages.iter() {
                let share = amount.checked_mul_floor(member.percentage)?;
                if !share.is_zero() {
                    // Check if this member has a preset distribution
                    if let Some(preset_dist) = preset_distributions.get(&member.addr) {
                        let mut preset_remaining = share;

                        // Apply the preset distribution to this member's share
                        for preset_member in preset_dist.member_percentages.iter() {
                            let preset_share = share.checked_mul_floor(preset_member.percentage)?;
                            if !preset_share.is_zero() {
                                native_distributions
                                    .entry(&preset_member.addr)
                                    .or_default()
                                    .entry(denom.clone())
                                    .and_modify(|e| *e += preset_share)
                                    .or_insert(preset_share);
                                preset_remaining = preset_remaining.checked_sub(preset_share)?;
                            }
                        }

                        // Handle preset remainder
                        if !preset_remaining.is_zero() {
                            native_distributions
                                .entry(&preset_dist.remainder_addr)
                                .or_default()
                                .entry(denom.clone())
                                .and_modify(|e| *e += preset_remaining)
                                .or_insert(preset_remaining);
                        }
                    } else {
                        // No preset distribution, handle normally
                        native_distributions
                            .entry(&member.addr)
                            .or_default()
                            .entry(denom.clone())
                            .and_modify(|e| *e += share)
                            .or_insert(share);
                    }
                    remaining_amount = remaining_amount.checked_sub(share)?;
                }
            }

            // Add remainder to remainder_addr's amount
            if !remaining_amount.is_zero() {
                native_distributions
                    .entry(&distribution.remainder_addr)
                    .or_default()
                    .entry(denom)
                    .and_modify(|e| *e += remaining_amount)
                    .or_insert(remaining_amount);
            }
        }

        for (addr, amount) in total_cw20.into_iter() {
            if amount.is_zero() {
                continue;
            }

            let mut remaining_amount = amount;

            // Member percentages
            for member in distribution.member_percentages.iter() {
                let share = amount.checked_mul_floor(member.percentage)?;
                if !share.is_zero() {
                    // Check if this member has a preset distribution
                    if let Some(preset_dist) = preset_distributions.get(&member.addr) {
                        let mut preset_remaining = share;

                        // Apply the preset distribution to this member's share
                        for preset_member in preset_dist.member_percentages.iter() {
                            let preset_share = share.checked_mul_floor(preset_member.percentage)?;
                            if !preset_share.is_zero() {
                                cw20_distributions
                                    .entry(&preset_member.addr)
                                    .or_default()
                                    .entry(addr.clone())
                                    .and_modify(|e| *e += preset_share)
                                    .or_insert(preset_share);
                                preset_remaining = preset_remaining.checked_sub(preset_share)?;
                            }
                        }

                        // Handle preset remainder
                        if !preset_remaining.is_zero() {
                            cw20_distributions
                                .entry(&preset_dist.remainder_addr)
                                .or_default()
                                .entry(addr.clone())
                                .and_modify(|e| *e += preset_remaining)
                                .or_insert(preset_remaining);
                        }
                    } else {
                        // No preset distribution, handle normally
                        cw20_distributions
                            .entry(&member.addr)
                            .or_default()
                            .entry(addr.clone())
                            .and_modify(|e| *e += share)
                            .or_insert(share);
                    }
                    remaining_amount = remaining_amount.checked_sub(share)?;
                }
            }

            // Add remainder to remainder_addr's amount
            if !remaining_amount.is_zero() {
                cw20_distributions
                    .entry(&distribution.remainder_addr)
                    .or_default()
                    .entry(addr)
                    .and_modify(|e| *e += remaining_amount)
                    .or_insert(remaining_amount);
            }
        }
    }

    // Create messages
    for fee in fees.iter() {
        if let Some(coins) = native_distributions.get_mut(&fee.receiver) {
            msgs.extend(
                Assets::Native(
                    coins
                        .into_iter()
                        .map(|x| Coin {
                            denom: x.0.clone(),
                            amount: *x.1,
                        })
                        .collect(),
                )
                .transmit_all(
                    deps.as_ref(),
                    &fee.receiver,
                    fee.cw20_msg.clone(),
                    fee.cw721_msg.clone(),
                )?,
            );
        }
        if let Some(cw20_coins) = cw20_distributions.get_mut(&fee.receiver) {
            msgs.extend(
                Assets::Cw20(
                    cw20_coins
                        .into_iter()
                        .map(|x| Cw20CoinVerified {
                            address: x.0.clone(),
                            amount: *x.1,
                        })
                        .collect(),
                )
                .transmit_all(
                    deps.as_ref(),
                    &fee.receiver,
                    fee.cw20_msg.clone(),
                    fee.cw721_msg.clone(),
                )?,
            );
        }
        native_distributions.remove(&fee.receiver);
        cw20_distributions.remove(&fee.receiver);
    }

    NATIVE_BALANCES.clear(deps.storage);
    CW20_BALANCES.clear(deps.storage);
    DUES.clear(deps.storage);
    IS_LOCKED.save(deps.storage, &false)?;
    HAS_DISTRIBUTED.save(deps.storage, &true)?;

    for (addr, coins) in native_distributions {
        for (denom, amount) in coins {
            NATIVE_BALANCES.save(deps.storage, (addr, &denom), &amount)?;
        }
    }
    for (addr, cw20_coins) in cw20_distributions {
        for (token, amount) in cw20_coins {
            CW20_BALANCES.save(deps.storage, (addr, &token), &amount)?;
        }
    }

    Ok(Response::new()
        .add_attribute("action", "distribute_tokens")
        .add_messages(msgs))
}

pub fn lock(deps: DepsMut, info: MessageInfo, value: bool) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    // Save the locked state to storage
    IS_LOCKED.save(deps.storage, &value)?;

    // Build and return the response
    Ok(Response::new()
        .add_attribute("action", "lock")
        .add_attribute("is_locked", value.to_string()))
}
