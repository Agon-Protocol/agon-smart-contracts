use std::iter;

use arena_interface::{
    escrow::{EnrollmentWithdrawMsg, TransferEscrowOwnershipMsg},
    fees::FeeInformation,
    group,
};
use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, DepsMut, Empty, Env, MessageInfo,
    Order, Response, StdError, StdResult, Uint128,
};
use cw20::{Cw20CoinVerified, Cw20ReceiveMsg};
use cw721::Cw721ReceiveMsg;
use cw_balance::{
    BalanceError, BalanceVerified, Cw721CollectionVerified, Distribution, MemberBalanceChecked,
    MemberBalanceUnchecked,
};
use cw_ownable::{assert_owner, get_ownership};

use crate::{
    query::is_locked,
    state::{
        is_fully_funded, BALANCE, DUE, HAS_DISTRIBUTED, INITIAL_DUE, IS_ENROLLMENT, IS_LOCKED,
        TOTAL_BALANCE,
    },
    ContractError,
};

pub fn set_dues(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dues: Vec<MemberBalanceUnchecked>,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        assert_owner(deps.storage, &info.sender)?;
    }

    IS_LOCKED.save(deps.storage, &false)?;
    for member_balance in dues {
        let member_balance = member_balance.into_checked(deps.as_ref())?;

        if INITIAL_DUE.has(deps.storage, &member_balance.addr) {
            return Err(ContractError::StdError(
                cosmwasm_std::StdError::GenericErr {
                    msg: "Cannot have duplicate addresses in dues".to_string(),
                },
            ));
        }

        INITIAL_DUE.save(deps.storage, &member_balance.addr, &member_balance.balance)?;
        DUE.save(deps.storage, &member_balance.addr, &member_balance.balance)?;
    }

    Ok(Response::new().add_attribute("action", "set_dues"))
}

pub fn withdraw(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Option<Binary>,
    cw721_msg: Option<Binary>,
    enrollment_withdraw_info: Option<EnrollmentWithdrawMsg>,
) -> Result<Response, ContractError> {
    if is_locked(deps.as_ref()) {
        return Err(ContractError::Locked {});
    }

    let mut msgs = vec![];
    // Load entire user balance
    let mut balance = BALANCE.load(deps.storage, &info.sender)?;

    // Load the total balance
    let mut total_balance = TOTAL_BALANCE.may_load(deps.storage)?.unwrap_or_default();

    if IS_ENROLLMENT.may_load(deps.storage)?.unwrap_or_default() {
        let ownership = get_ownership(deps.storage)?;

        // Only the enrollment contract can trigger withdrawals
        // This config is changed when the escrow is transferred to the competition modules
        if ownership.owner.map_or(true, |x| x != info.sender) {
            return Err(ContractError::EnrollmentWithdraw {});
        }

        match enrollment_withdraw_info {
            Some(EnrollmentWithdrawMsg { addrs, entry_fee }) => {
                let addrs = addrs
                    .into_iter()
                    .map(|addr| {
                        let validated_addr = deps.api.addr_validate(&addr)?; // Validate the address

                        // Create and push the message for this address
                        msgs.push(CosmosMsg::Bank(BankMsg::Send {
                            to_address: validated_addr.to_string(),
                            amount: vec![entry_fee.clone()],
                        }));

                        Ok(validated_addr) // Return the validated address
                    })
                    .collect::<StdResult<Vec<_>>>()?;

                // Deduct the full entry fee balance
                let full_entry_fee_balance = BalanceVerified {
                    native: Some(vec![Coin {
                        denom: entry_fee.denom.clone(),
                        amount: entry_fee
                            .amount
                            .checked_mul(Uint128::new(addrs.len() as u128))?,
                    }]),
                    cw20: None,
                    cw721: None,
                };
                balance = balance.checked_sub(&full_entry_fee_balance)?;
                total_balance = total_balance.checked_sub(&full_entry_fee_balance)?;

                // Save balance
                if balance.is_empty() {
                    BALANCE.remove(deps.storage, &info.sender);
                } else {
                    BALANCE.save(deps.storage, &info.sender, &balance)?;
                }
            }
            None => {
                return Err(ContractError::StdError(StdError::generic_err(
                    "Enrollment withdraw msg is required when configured for enrollment contracts",
                )))
            }
        };
    } else if balance.is_empty() {
        BALANCE.remove(deps.storage, &info.sender);
    } else {
        // Update total balance and related storage entries
        total_balance = total_balance.checked_sub(&balance)?;

        if !HAS_DISTRIBUTED.may_load(deps.storage)?.unwrap_or_default() {
            // Set due to the initial due
            if let Some(initial_due) = &INITIAL_DUE.may_load(deps.storage, &info.sender)? {
                DUE.save(deps.storage, &info.sender, initial_due)?;
            }
        }

        // Clear balance
        BALANCE.remove(deps.storage, &info.sender);

        msgs = balance.transmit_all(
            deps.as_ref(),
            &info.sender,
            cw20_msg.clone(),
            cw721_msg.clone(),
        )?;
    };

    // Update or remove total balance
    if total_balance.is_empty() {
        TOTAL_BALANCE.remove(deps.storage);
    } else {
        TOTAL_BALANCE.save(deps.storage, &total_balance)?;
    }

    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_attribute("addr", info.sender)
        .add_messages(msgs))
}

// This function receives native tokens and updates the balance
pub fn receive_native(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let balance = BalanceVerified {
        native: Some(info.funds),
        cw20: None,
        cw721: None,
    };

    receive_balance(deps, info.sender, balance)
}

// This function receives CW20 tokens and updates the balance
pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender_addr = deps.api.addr_validate(&cw20_receive_msg.sender)?;
    let cw20_balance = vec![Cw20CoinVerified {
        address: info.sender,
        amount: cw20_receive_msg.amount,
    }];

    let balance = BalanceVerified {
        native: Some(info.funds),
        cw20: Some(cw20_balance),
        cw721: None,
    };

    receive_balance(deps, sender_addr, balance)
}

// This function receives CW721 tokens and updates the balance
pub fn receive_cw721(
    deps: DepsMut,
    info: MessageInfo,
    cw721_receive_msg: Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender_addr = deps.api.addr_validate(&cw721_receive_msg.sender)?;
    let cw721_balance = vec![Cw721CollectionVerified {
        address: info.sender,
        token_ids: vec![cw721_receive_msg.token_id],
    }];

    let balance = BalanceVerified {
        native: Some(info.funds),
        cw20: None,
        cw721: Some(cw721_balance),
    };

    receive_balance(deps, sender_addr, balance)
}

fn receive_balance(
    deps: DepsMut,
    addr: Addr,
    balance: BalanceVerified,
) -> Result<Response, ContractError> {
    if balance.is_empty() {
        return Err(ContractError::EmptyBalance {});
    }
    if HAS_DISTRIBUTED.exists(deps.storage) {
        return Err(ContractError::AlreadyDistributed {});
    }

    // Update the stored balance for the given address
    let updated_balance =
        BALANCE.update(deps.storage, &addr, |existing_balance| -> StdResult<_> {
            existing_balance.unwrap_or_default().checked_add(&balance)
        })?;
    let mut msgs: Vec<CosmosMsg> = vec![];

    // Check if the address has a due balance
    if let Some(due_balance) = DUE.may_load(deps.storage, &addr)? {
        let remaining_due = updated_balance.difference_to(&due_balance)?;

        // Handle the case where the due balance is fully paid
        if remaining_due.is_empty() {
            DUE.remove(deps.storage, &addr);

            // Lock if fully funded and send activation message if needed
            if is_fully_funded(deps.as_ref()) {
                IS_LOCKED.save(deps.storage, &true)?;

                if let Some(owner) = get_ownership(deps.storage)?.owner {
                    msgs.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
                        contract_addr: owner.to_string(),
                        msg: to_json_binary(
                            &arena_interface::competition::msg::ExecuteBase::ActivateCompetition::<
                                Empty,
                                Empty,
                            > {},
                        )?,
                        funds: vec![],
                    }));
                }
            }
        } else {
            DUE.save(deps.storage, &addr, &remaining_due)?;
        }
    }

    // Update the total balance in storage
    if TOTAL_BALANCE.exists(deps.storage) {
        TOTAL_BALANCE.update(deps.storage, |total| total.checked_add(&balance))?;
    } else {
        TOTAL_BALANCE.save(deps.storage, &balance)?;
    }

    Ok(Response::new()
        .add_attribute("action", "receive_balance")
        .add_attribute("balance", updated_balance.to_string())
        .add_messages(msgs))
}

pub fn distribute(
    deps: DepsMut,
    info: MessageInfo,
    distribution: Option<Distribution<String>>,
    layered_fees: Option<Vec<FeeInformation<String>>>,
    activation_height: Option<u64>,
    group_contract: String,
) -> Result<Response, ContractError> {
    // Ensure the sender is the owner
    assert_owner(deps.storage, &info.sender)?;

    // Validate the group contract
    let group_contract = deps.api.addr_validate(&group_contract)?;

    // Load the total balance available for distribution
    let mut total_balance = TOTAL_BALANCE.load(deps.storage)?;

    let mut msgs = vec![];
    let mut attrs = vec![];

    // Process layered fees if provided
    if let Some(layered_fees) = layered_fees.as_ref() {
        // Validate the tax info
        let validated_layered_fees: Vec<FeeInformation<Addr>> = layered_fees
            .iter()
            .map(|fee| fee.into_checked(deps.as_ref()))
            .collect::<StdResult<_>>()?;

        // Process each fee
        for fee in validated_layered_fees {
            let fee_amounts = total_balance.checked_mul_floor(fee.tax)?;

            // Update total balance
            total_balance = TOTAL_BALANCE.update(deps.storage, |x| -> Result<_, BalanceError> {
                x.checked_sub(&fee_amounts)
            })?;

            // Add messages for fee transmission if amounts are not empty
            if !fee_amounts.is_empty() {
                msgs.extend(fee_amounts.transmit_all(
                    deps.as_ref(),
                    &fee.receiver,
                    fee.cw20_msg,
                    fee.cw721_msg,
                )?);
                attrs.push(("Fee", fee.receiver.to_string()));
            }
        }
    }

    // Process distribution if provided
    let distributed_amounts = if let Some(distribution) = distribution {
        let distribution = distribution.into_checked(deps.as_ref())?;

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

        // Calculate the distribution amounts based on the total balance and distribution
        let distributed_amounts = total_balance.split(&distribution)?;

        // Clear existing balance storage
        BALANCE.clear(deps.storage);

        distributed_amounts
    } else {
        let mut distributed_amounts = vec![];
        for (addr, balance) in BALANCE
            .range(deps.storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()?
        {
            let mut new_balance = balance.clone();
            if let Some(layered_fees) = layered_fees.as_ref() {
                for fee in layered_fees {
                    new_balance = balance.checked_sub(&balance.checked_mul_floor(fee.tax)?)?;
                }
            }

            distributed_amounts.push(MemberBalanceChecked {
                addr,
                balance: new_balance,
            });
        }

        // Clear existing balance storage
        BALANCE.clear(deps.storage);

        distributed_amounts
    };

    // Query payment registry
    let payment_registry: Option<String> = deps.querier.query_wasm_smart(
        info.sender.to_string(),
        &arena_interface::competition::msg::QueryBase::PaymentRegistry::<Empty, Empty, Empty> {},
    )?;
    let payment_registry = payment_registry
        .map(|x| deps.api.addr_validate(&x))
        .transpose()?;
    let mut has_preset_distribution = false;

    // Process each distributed amount
    for distributed_amount in distributed_amounts {
        if let Some(ref payment_registry) = payment_registry {
            // Query preset distribution from payment registry
            let preset_distribution: Option<Distribution<String>> = deps.querier.query_wasm_smart(
                payment_registry.to_string(),
                &arena_interface::registry::QueryMsg::GetDistribution {
                    addr: distributed_amount.addr.to_string(),
                    height: activation_height,
                },
            )?;

            if let Some(preset_distribution) = preset_distribution {
                let preset_distribution = preset_distribution.into_checked(deps.as_ref())?;
                let new_balances = distributed_amount.balance.split(&preset_distribution)?;
                has_preset_distribution = true;

                // Update balances based on preset distribution
                for new_balance in new_balances {
                    BALANCE.update(
                        deps.storage,
                        &new_balance.addr,
                        |old_balance| -> Result<_, ContractError> {
                            match old_balance {
                                Some(old_balance) => {
                                    Ok(old_balance.checked_add(&new_balance.balance)?)
                                }
                                None => Ok(new_balance.balance),
                            }
                        },
                    )?;
                }
            }
        }

        if !has_preset_distribution {
            // Update balance directly if no preset distribution
            BALANCE.update(
                deps.storage,
                &distributed_amount.addr,
                |old_balance| -> Result<_, ContractError> {
                    match old_balance {
                        Some(old_balance) => {
                            Ok(old_balance.checked_add(&distributed_amount.balance)?)
                        }
                        None => Ok(distributed_amount.balance),
                    }
                },
            )?;
        }
    }

    // Update contract state
    IS_LOCKED.save(deps.storage, &false)?;
    HAS_DISTRIBUTED.save(deps.storage, &true)?;
    DUE.clear(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "distribute")
        .add_attributes(attrs)
        .add_messages(msgs))
}

pub fn lock(
    deps: DepsMut,
    info: MessageInfo,
    value: bool,
    transfer_ownership: Option<TransferEscrowOwnershipMsg>,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    // Save the locked state to storage
    IS_LOCKED.save(deps.storage, &value)?;

    let mut res = Response::new()
        .add_attribute("action", "lock")
        .add_attribute("is_locked", value.to_string());

    // Set new owner if provided
    if let Some(new_ownership) = transfer_ownership {
        let ownership =
            cw_ownable::initialize_owner(deps.storage, deps.api, Some(&new_ownership.addr))?;
        // Assume a transfership means we're moving from enrollment contract to
        IS_ENROLLMENT.save(deps.storage, &new_ownership.is_enrollment)?;
        res = res.add_attributes(ownership.into_attributes());
    }

    Ok(res)
}
