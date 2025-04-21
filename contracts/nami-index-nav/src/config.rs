use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, StdResult, Storage};
use cw_storage_plus::Item;
use nami_rs::{index_nav::InstantiateMsg, OracleConfig};

use crate::{asset_allocation::AssetAllocation, ContractError};

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub base_denom: String,
    pub fee_collector: Addr,
    pub allocations: Vec<AssetAllocation<OracleConfig>>,
}
//TODO: Must verify that every target_denom is unique and valid with a valid oracle price
impl Config {
    pub fn new(api: &dyn Api, msg: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            base_denom: msg.base_denom,
            fee_collector: api.addr_validate(&msg.fee_collector)?,
            allocations: msg
                .target_denoms
                .iter()
                .map(|(a, b, c, d)| {
                    Ok(AssetAllocation {
                        denom: a.clone(),
                        weight: b.clone(),
                        swap_contract: Some(api.addr_validate(c)?),
                        oracle: d.clone(),
                    })
                })
                .collect::<StdResult<Vec<AssetAllocation<OracleConfig>>>>()?,
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
