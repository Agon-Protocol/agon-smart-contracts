use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, OverflowError,
    OverflowOperation, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg};
use cw721::Cw721ExecuteMsg;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::{
    is_contract, BalanceError, Cw721Collection, Cw721CollectionVerified, MemberShareVerified,
};

#[cw_serde]
pub struct MemberBalanceVerified {
    pub addr: Addr,
    pub balance: BalanceVerified,
}

#[cw_serde]
pub struct MemberBalance {
    pub addr: String,
    pub balance: Balance,
}

impl MemberBalance {
    pub fn to_verified(self, deps: Deps) -> StdResult<MemberBalanceVerified> {
        Ok(MemberBalanceVerified {
            addr: deps.api.addr_validate(&self.addr)?,
            balance: self.balance.to_verified(deps)?,
        })
    }
}

#[cw_serde]
pub struct Balance {
    pub native: Vec<Coin>,
    pub cw20: Vec<Cw20Coin>,
    pub cw721: Vec<Cw721Collection>,
}

#[cw_serde]
pub enum TokenType {
    Native,
    Cw20,
    Cw721,
}

impl Balance {
    pub fn to_verified(self, deps: Deps) -> StdResult<BalanceVerified> {
        Ok(BalanceVerified {
            native: self.native,
            cw20: self
                .cw20
                .iter()
                .map(|x| {
                    Ok(Cw20CoinVerified {
                        address: deps.api.addr_validate(&x.address)?,
                        amount: x.amount,
                    })
                })
                .collect::<StdResult<Vec<Cw20CoinVerified>>>()?,
            cw721: self
                .cw721
                .iter()
                .map(|x| {
                    Ok(Cw721CollectionVerified {
                        addr: deps.api.addr_validate(&x.addr)?,
                        token_ids: x.token_ids.clone(),
                    })
                })
                .collect::<StdResult<Vec<Cw721CollectionVerified>>>()?,
        })
    }
}

#[cw_serde]
#[derive(Default)]
pub struct BalanceVerified {
    pub native: Vec<Coin>,
    pub cw20: Vec<Cw20CoinVerified>,
    pub cw721: Vec<Cw721CollectionVerified>,
}

impl Display for BalanceVerified {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "Native:")?;
        for coin in &self.native {
            writeln!(f, "  {}: {}", coin.denom, coin.amount)?;
        }

        writeln!(f, "CW20:")?;
        for cw20_coin in &self.cw20 {
            writeln!(f, "  {}: {}", cw20_coin.address, cw20_coin.amount)?;
        }

        writeln!(f, "CW721:")?;
        for cw721_tokens in &self.cw721 {
            writeln!(f, "  {}: {:?}", cw721_tokens.addr, cw721_tokens.token_ids)?;
        }

        Ok(())
    }
}

impl BalanceVerified {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.native.is_empty() && self.cw20.is_empty() && self.cw721.is_empty()
    }

    pub fn is_ge(&self, other: &BalanceVerified) -> bool {
        // Helper function for comparing native tokens
        fn native_ge(self_native: &[Coin], other_native: &[Coin]) -> bool {
            let self_map: HashMap<&str, &Coin> = self_native
                .iter()
                .map(|coin| (coin.denom.as_str(), coin))
                .collect();

            for other_coin in other_native {
                if let Some(self_coin) = self_map.get(&other_coin.denom.as_str()) {
                    if self_coin.amount < other_coin.amount {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }

        // Helper function for comparing cw20 tokens
        fn cw20_ge(self_cw20: &[Cw20CoinVerified], other_cw20: &[Cw20CoinVerified]) -> bool {
            let self_map: HashMap<&Addr, &Cw20CoinVerified> =
                self_cw20.iter().map(|coin| (&coin.address, coin)).collect();

            for other_coin in other_cw20 {
                if let Some(self_coin) = self_map.get(&other_coin.address) {
                    if self_coin.amount < other_coin.amount {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }

        // Helper function for comparing cw721 tokens
        fn cw721_ge(
            self_cw721: &[Cw721CollectionVerified],
            other_cw721: &[Cw721CollectionVerified],
        ) -> bool {
            let self_map: HashMap<&Addr, &Vec<String>> = self_cw721
                .iter()
                .map(|collection| (&collection.addr, &collection.token_ids))
                .collect();

            for other_collection in other_cw721 {
                if let Some(self_token_ids) = self_map.get(&other_collection.addr) {
                    let self_token_ids: HashSet<&String> = self_token_ids.iter().collect();

                    for token_id in &other_collection.token_ids {
                        if !self_token_ids.contains(token_id) {
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            }
            true
        }

        let native_ge_result = native_ge(&self.native, &other.native);
        let cw20_ge_result = cw20_ge(&self.cw20, &other.cw20);
        let cw721_ge_result = cw721_ge(&self.cw721, &other.cw721);

        native_ge_result && cw20_ge_result && cw721_ge_result
    }

    pub fn get_amount(&self, token_type: TokenType, identifier: &str) -> Option<Uint128> {
        match token_type {
            TokenType::Native => self
                .native
                .iter()
                .find(|coin| coin.denom == identifier)
                .map(|coin| coin.amount),
            TokenType::Cw20 => self
                .cw20
                .iter()
                .find(|cw20_coin| cw20_coin.address == identifier)
                .map(|cw20_coin| cw20_coin.amount),
            TokenType::Cw721 => {
                if self
                    .cw721
                    .iter()
                    .any(|cw721_tokens| cw721_tokens.token_ids.contains(&identifier.to_string()))
                {
                    Some(Uint128::one())
                } else {
                    None
                }
            }
        }
    }

    pub fn checked_add(&self, other: &BalanceVerified) -> StdResult<BalanceVerified> {
        if self.is_empty() {
            return Ok(other.to_owned());
        }
        if other.is_empty() {
            return Ok(self.to_owned());
        }

        let mut native_map: HashMap<String, Uint128> = self
            .native
            .iter()
            .map(|coin| (coin.denom.clone(), coin.amount))
            .collect();
        for coin in &other.native {
            let entry = native_map
                .entry(coin.denom.clone())
                .or_insert(Uint128::zero());
            *entry = entry.checked_add(coin.amount)?;
        }

        let mut cw20_map: HashMap<Addr, Uint128> = self
            .cw20
            .iter()
            .map(|coin| (coin.address.clone(), coin.amount))
            .collect();
        for coin in &other.cw20 {
            let entry = cw20_map
                .entry(coin.address.clone())
                .or_insert(Uint128::zero());
            *entry = entry.checked_add(coin.amount)?;
        }

        let mut cw721_map: HashMap<Addr, HashSet<String>> = self
            .cw721
            .iter()
            .map(|token| {
                (
                    token.addr.clone(),
                    token.token_ids.iter().cloned().collect(),
                )
            })
            .collect();

        for token in &other.cw721 {
            let entry = cw721_map
                .entry(token.addr.clone())
                .or_insert_with(HashSet::new);

            for token_id in &token.token_ids {
                // If the token_id is already present, it's a duplicate and we return an error.
                if !entry.insert(token_id.clone()) {
                    return Err(cosmwasm_std::StdError::Overflow {
                        source: OverflowError::new(OverflowOperation::Add, self, other),
                    });
                }
            }
        }

        Ok(BalanceVerified {
            native: native_map
                .into_iter()
                .map(|(denom, amount)| Coin { denom, amount })
                .collect(),
            cw20: cw20_map
                .into_iter()
                .map(|(address, amount)| Cw20CoinVerified { address, amount })
                .collect(),
            cw721: cw721_map
                .into_iter()
                .map(|(addr, token_ids)| Cw721CollectionVerified {
                    addr,
                    token_ids: token_ids.into_iter().collect(),
                })
                .collect(),
        })
    }

    pub fn checked_sub(&self, other: &BalanceVerified) -> StdResult<BalanceVerified> {
        if other.is_empty() {
            return Ok(self.to_owned());
        }

        let mut native_map: HashMap<String, Uint128> = self
            .native
            .iter()
            .map(|coin| (coin.denom.clone(), coin.amount))
            .collect();
        for coin in &other.native {
            match native_map.entry(coin.denom.clone()) {
                Entry::Occupied(mut entry) => {
                    let total_amount = entry.get_mut().checked_sub(coin.amount)?;
                    if total_amount.is_zero() {
                        entry.remove();
                    } else {
                        *entry.get_mut() = total_amount;
                    }
                }

                Entry::Vacant(_) => {
                    return Err(cosmwasm_std::StdError::Overflow {
                        source: cosmwasm_std::OverflowError::new(
                            OverflowOperation::Sub,
                            self,
                            other,
                        ),
                    });
                }
            }
        }

        let mut cw20_map: HashMap<Addr, Uint128> = self
            .cw20
            .iter()
            .map(|coin| (coin.address.clone(), coin.amount))
            .collect();
        for coin in &other.cw20 {
            match cw20_map.entry(coin.address.clone()) {
                Entry::Occupied(mut entry) => {
                    let total_amount = entry.get_mut().checked_sub(coin.amount)?;
                    if total_amount.is_zero() {
                        entry.remove();
                    } else {
                        *entry.get_mut() = total_amount;
                    }
                }

                Entry::Vacant(_) => {
                    return Err(cosmwasm_std::StdError::Overflow {
                        source: cosmwasm_std::OverflowError::new(
                            OverflowOperation::Sub,
                            self,
                            other,
                        ),
                    });
                }
            }
        }

        let mut cw721_map: HashMap<Addr, HashSet<String>> = self
            .cw721
            .iter()
            .map(|token| {
                (
                    token.addr.clone(),
                    token.token_ids.iter().cloned().collect(),
                )
            })
            .collect();
        for token in &other.cw721 {
            if let Some(entry_set) = cw721_map.get_mut(&token.addr) {
                for token_id in &token.token_ids {
                    // Removes the token_id from the set if it exists; no-op if it doesn't
                    if !entry_set.remove(token_id) {
                        // Return error if a token_id is missing
                        return Err(cosmwasm_std::StdError::Overflow {
                            source: OverflowError::new(OverflowOperation::Sub, self, other),
                        });
                    }
                }
            } else {
                // Return error if a corresponding addr is missing
                return Err(cosmwasm_std::StdError::Overflow {
                    source: OverflowError::new(OverflowOperation::Sub, self, other),
                });
            }
        }

        Ok(BalanceVerified {
            native: native_map
                .into_iter()
                .map(|(denom, amount)| Coin { denom, amount })
                .collect(),
            cw20: cw20_map
                .into_iter()
                .map(|(address, amount)| Cw20CoinVerified { address, amount })
                .collect(),
            cw721: cw721_map
                .into_iter()
                .map(|(addr, tokens)| Cw721CollectionVerified {
                    addr,
                    token_ids: tokens.into_iter().collect(),
                })
                .collect(),
        })
    }

    pub fn transmit_all(
        &self,
        deps: Deps,
        recipient: &Addr,
        cw20_msg: Option<Binary>,
        cw721_msg: Option<Binary>,
    ) -> StdResult<Vec<CosmosMsg>> {
        match is_contract(deps, recipient.to_string()) {
            false => self.transfer_all(recipient),
            true => self.send_all(recipient, cw20_msg, cw721_msg),
        }
    }

    pub fn transfer_all(&self, recipient: &Addr) -> StdResult<Vec<CosmosMsg>> {
        let mut messages: Vec<CosmosMsg> = Vec::new();

        // Send native tokens
        messages.extend(self.send_native(recipient.to_string()));

        // Send CW20 tokens
        messages.extend(self.transfer_cw20(recipient.to_string())?);

        // Send CW721 tokens
        messages.extend(self.transfer_cw721(recipient.to_string())?);

        Ok(messages)
    }

    pub fn send_all(
        &self,
        contract_addr: &Addr,
        cw20_msg: Option<Binary>,
        cw721_msg: Option<Binary>,
    ) -> StdResult<Vec<CosmosMsg>> {
        let mut messages: Vec<CosmosMsg> = Vec::new();

        // Send native tokens
        messages.extend(self.send_native(contract_addr.to_string()));

        // Send CW20 tokens
        messages.extend(self.send_cw20(contract_addr.to_string(), cw20_msg.unwrap_or_default())?);

        // Send CW721 tokens
        messages.extend(self.send_cw721(contract_addr.to_string(), cw721_msg.unwrap_or_default())?);

        Ok(messages)
    }

    pub fn send_native(&self, to_address: String) -> Vec<CosmosMsg> {
        if self.native.is_empty() {
            vec![]
        } else {
            vec![CosmosMsg::Bank(BankMsg::Send {
                to_address,
                amount: self.native.clone(),
            })]
        }
    }

    pub fn send_cw20(&self, contract: String, msg: Binary) -> StdResult<Vec<CosmosMsg>> {
        self.cw20
            .iter()
            .map(|cw20_coin| {
                let exec_msg = WasmMsg::Execute {
                    contract_addr: cw20_coin.address.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: contract.clone(),
                        amount: cw20_coin.amount,
                        msg: msg.clone(),
                    })?,
                    funds: vec![],
                };
                Ok(CosmosMsg::Wasm(exec_msg))
            })
            .collect()
    }

    pub fn send_cw721(&self, contract: String, msg: Binary) -> StdResult<Vec<CosmosMsg>> {
        self.cw721
            .iter()
            .flat_map(|cw721_collection| {
                let contract = contract.clone();
                let msg = msg.clone();
                cw721_collection.token_ids.iter().map(move |token_id| {
                    let exec_msg = WasmMsg::Execute {
                        contract_addr: cw721_collection.addr.to_string(),
                        msg: to_binary(&Cw721ExecuteMsg::SendNft {
                            contract: contract.clone(),
                            token_id: token_id.clone(),
                            msg: msg.clone(),
                        })?,
                        funds: vec![],
                    };
                    Ok(CosmosMsg::Wasm(exec_msg))
                })
            })
            .collect()
    }

    pub fn transfer_cw721(&self, recipient: String) -> StdResult<Vec<CosmosMsg>> {
        self.cw721
            .iter()
            .flat_map(|cw721_collection| {
                let recipient = recipient.clone();
                cw721_collection.token_ids.iter().map(move |token_id| {
                    let exec_msg = WasmMsg::Execute {
                        contract_addr: cw721_collection.addr.to_string(),
                        msg: to_binary(&Cw721ExecuteMsg::TransferNft {
                            recipient: recipient.to_string(),
                            token_id: token_id.clone(),
                        })?,
                        funds: vec![],
                    };
                    Ok(CosmosMsg::Wasm(exec_msg))
                })
            })
            .collect()
    }

    pub fn transfer_cw20(&self, recipient: String) -> StdResult<Vec<CosmosMsg>> {
        self.cw20
            .iter()
            .map(|cw20_coin| {
                let exec_msg = WasmMsg::Execute {
                    contract_addr: cw20_coin.address.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: recipient.clone(),
                        amount: cw20_coin.amount,
                    })?,
                    funds: vec![],
                };
                Ok(CosmosMsg::Wasm(exec_msg))
            })
            .collect()
    }

    /// Splits a given `Balance` among multiple users based on their assigned weights.
    ///
    /// # Arguments
    ///
    /// * `weights` - A reference to a `HashMap` containing user addresses and their corresponding weights as `u128` values.
    /// * `remainder_address` - A reference to the address that will receive any remaining tokens after the split, as well as all NFTs.
    ///
    /// # Returns
    ///
    /// A `StdResult` containing a `HashMap` of user addresses mapped to their respective split `Balance` instances.
    ///
    /// # Errors
    ///
    /// This function may return an error if any of the following occurs:
    ///
    /// * Division by zero when calculating the weight fraction.
    /// * Multiplication overflow when calculating the split amounts for native and CW20 tokens.
    /// * Subtraction underflow when updating the remainders for native and CW20 tokens.
    pub fn split(
        &self,
        distribution: &Vec<MemberShareVerified>,
        remainder_address: &Addr,
    ) -> Result<Vec<MemberBalanceVerified>, BalanceError> {
        let total_weight = distribution
            .iter()
            .try_fold(Uint128::zero(), |accumulator, x| {
                accumulator.checked_add(x.shares)
            })?;
        let mut split_balances: Vec<MemberBalanceVerified> = Vec::new();

        let mut remainders_native: HashMap<String, Uint128> = self
            .native
            .iter()
            .map(|x| (x.denom.clone(), x.amount))
            .collect();
        let mut remainders_cw20: HashMap<Addr, Uint128> = self
            .cw20
            .iter()
            .map(|x| (x.address.clone(), x.amount))
            .collect();

        for member_share in distribution {
            let weight_fraction = Decimal::from_ratio(member_share.shares, total_weight);

            let mut split_native = HashMap::new();
            for coin in &self.native {
                let decimal_amount = Decimal::from_atomics(coin.amount, 0u32)?;
                let split_amount = weight_fraction.checked_mul(decimal_amount)?.to_uint_floor();

                // Deduct the split amount from the remainder
                if let Some(remainder) = remainders_native.get_mut(&coin.denom) {
                    *remainder = remainder.checked_sub(split_amount)?;
                }

                split_native.insert(coin.denom.clone(), split_amount);
            }

            let mut split_cw20 = HashMap::new();
            for cw20_coin in &self.cw20 {
                let decimal_amount = Decimal::from_atomics(cw20_coin.amount, 0u32)?;
                let split_amount = weight_fraction.checked_mul(decimal_amount)?.to_uint_floor();

                // Deduct the split amount from the remainder
                if let Some(remainder) = remainders_cw20.get_mut(&cw20_coin.address) {
                    *remainder = remainder.checked_sub(split_amount)?;
                }

                split_cw20.insert(cw20_coin.address.clone(), split_amount);
            }

            let split_balance = BalanceVerified {
                native: split_native
                    .into_iter()
                    .map(|(denom, amount)| Coin { denom, amount })
                    .collect(),
                cw20: split_cw20
                    .into_iter()
                    .map(|(address, amount)| Cw20CoinVerified { address, amount })
                    .collect(),
                cw721: vec![],
            };

            let member_balance = MemberBalanceVerified {
                addr: member_share.addr.clone(),
                balance: split_balance,
            };

            split_balances.push(member_balance);
        }

        // Apply the remainder_balance to the corresponding split_balances entry
        let remainder_balance = BalanceVerified {
            native: remainders_native
                .into_iter()
                .map(|(denom, amount)| Coin { denom, amount })
                .collect(),
            cw20: remainders_cw20
                .into_iter()
                .map(|(address, amount)| Cw20CoinVerified { address, amount })
                .collect(),
            cw721: self.cw721.clone(),
        };

        let remainder_index = split_balances
            .iter()
            .position(|member_balance| member_balance.addr == *remainder_address);

        if let Some(index) = remainder_index {
            let remainder_member_balance = &mut split_balances[index];
            remainder_member_balance.balance = remainder_member_balance
                .balance
                .checked_add(&remainder_balance)?;
        } else {
            let remainder_member_balance = MemberBalanceVerified {
                addr: remainder_address.clone(),
                balance: remainder_balance,
            };
            split_balances.push(remainder_member_balance);
        }
        Ok(split_balances)
    }
}
