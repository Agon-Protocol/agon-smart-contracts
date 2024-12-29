use std::collections::BTreeMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, CoinsError, Deps};
use cw20::{Cw20Coin, Cw20CoinVerified};

use crate::BalanceError;

#[cw_serde]
pub struct Cw20Coins(BTreeMap<Addr, Cw20CoinVerified>);

impl Cw20Coins {
    pub fn into_vec(self) -> Vec<Cw20CoinVerified> {
        self.0.into_values().collect()
    }

    pub fn try_from_deps(deps: Deps, vec: Vec<Cw20Coin>) -> Result<Self, BalanceError> {
        let mut map = BTreeMap::new();
        for coin in vec {
            if coin.amount.is_zero() {
                continue;
            }

            let coin = Cw20CoinVerified {
                address: deps.api.addr_validate(&coin.address)?,
                amount: coin.amount,
            };

            // if the insertion returns a previous value, we have a duplicate denom
            if map.insert(coin.address.clone(), coin).is_some() {
                return Err(BalanceError::CoinsError(CoinsError::DuplicateDenom));
            }
        }

        Ok(Self(map))
    }
}
