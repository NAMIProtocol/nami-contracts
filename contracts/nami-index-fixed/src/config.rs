use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, StdResult, Storage, Uint128};
use cw_storage_plus::Item;
use nami_rs::index_fixed::InstantiateMsg;

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub base_denom: String,
    pub fee_collector: Addr,
    pub target_denoms: Vec<(String, Uint128, Addr)>,
}

impl Config {
    pub fn new(api: &dyn Api, msg: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            base_denom: msg.base_denom,
            fee_collector: api.addr_validate(&msg.fee_collector)?,
            target_denoms: msg
                .target_denoms
                .iter()
                .map(|(a, b, c)| Ok((a.clone(), *b, api.addr_validate(c)?)))
                .collect::<StdResult<Vec<(String, Uint128, Addr)>>>()?,
        })
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }
}

#[cfg(test)]
mod tests {}
