use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, Coin, Coins, CosmosMsg, Deps, StdResult, WasmMsg,
};
use cw20::{Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg};

use crate::{cw20_coins::Cw20Coins, is_contract, BalanceError};

#[cw_serde]
pub enum Assets {
    Native(Vec<Coin>),
    Cw20(Vec<Cw20CoinVerified>),
}

impl Assets {
    /// Transmits all assets (native and CW20) to the specified recipient.
    pub fn transmit_all(
        self,
        deps: Deps,
        recipient: &Addr,
        cw20_msg: Option<Binary>,
        cw721_msg: Option<Binary>,
    ) -> StdResult<Vec<CosmosMsg>> {
        if is_contract(deps, recipient) {
            self.send_all(recipient, cw20_msg, cw721_msg)
        } else {
            self.transfer_all(recipient)
        }
    }

    /// Sends all assets to a contract address.
    fn send_all(
        self,
        contract_addr: &Addr,
        cw20_msg: Option<Binary>,
        _cw721_msg: Option<Binary>,
    ) -> StdResult<Vec<CosmosMsg>> {
        let mut messages = Vec::new();

        match self {
            Assets::Native(native) => {
                if !native.is_empty() {
                    messages.push(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                        to_address: contract_addr.to_string(),
                        amount: native,
                    }));
                }
            }
            Assets::Cw20(cw20) => {
                for cw20_coin in cw20 {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: cw20_coin.address.to_string(),
                        msg: to_json_binary(&Cw20ExecuteMsg::Send {
                            contract: contract_addr.to_string(),
                            amount: cw20_coin.amount,
                            msg: cw20_msg.clone().unwrap_or_default(),
                        })?,
                        funds: vec![],
                    }));
                }
            }
        }

        Ok(messages)
    }

    /// Transfers all assets to a recipient (non-contract address).
    fn transfer_all(self, recipient: &Addr) -> StdResult<Vec<CosmosMsg>> {
        let mut messages = Vec::new();

        match self {
            Assets::Native(native) => {
                if !native.is_empty() {
                    messages.push(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
                        to_address: recipient.to_string(),
                        amount: native,
                    }));
                }
            }
            Assets::Cw20(cw20) => {
                for cw20_coin in cw20 {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: cw20_coin.address.to_string(),
                        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                            recipient: recipient.to_string(),
                            amount: cw20_coin.amount,
                        })?,
                        funds: vec![],
                    }));
                }
            }
        }

        Ok(messages)
    }
}

#[cw_serde]
pub enum AssetsUnchecked {
    Native(Vec<Coin>),
    Cw20(Vec<Cw20Coin>),
}

impl AssetsUnchecked {
    pub fn into_checked(self, deps: Deps) -> Result<Assets, BalanceError> {
        match self {
            AssetsUnchecked::Native(vec) => {
                let coins = Coins::try_from(vec)?;

                Ok(Assets::Native(coins.into_vec()))
            }
            AssetsUnchecked::Cw20(vec) => {
                let coins = Cw20Coins::try_from_deps(deps, vec)?;

                Ok(Assets::Cw20(coins.into_vec()))
            }
        }
    }
}

#[cw_serde]
pub struct MemberAssets {
    pub addr: Addr,
    pub assets: Vec<Assets>,
}

#[cw_serde]
pub struct MemberAssetsUnchecked {
    pub addr: String,
    pub assets: Vec<AssetsUnchecked>,
}

impl MemberAssetsUnchecked {
    pub fn into_checked(self, deps: Deps) -> Result<MemberAssets, BalanceError> {
        Ok(MemberAssets {
            addr: deps.api.addr_validate(&self.addr)?,
            assets: self
                .assets
                .into_iter()
                .map(|x| x.into_checked(deps))
                .collect::<Result<Vec<_>, BalanceError>>()?,
        })
    }
}
