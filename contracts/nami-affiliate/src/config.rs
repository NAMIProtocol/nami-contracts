use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, StdResult, Storage};
use cw_storage_plus::Item;
use nami_rs::affiliate::InstantiateMsg;

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub max_affiliate_fee_bps: u16,
}

impl From<InstantiateMsg> for Config {
    fn from(msg: InstantiateMsg) -> Self {
        Self {
            max_affiliate_fee_bps: msg.max_affiliate_fee_bps,
        }
    }
}

impl Config {
    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }

    pub fn update(&mut self, bps: u16) -> Result<(), ContractError> {
        self.max_affiliate_fee_bps = bps;
        self.validate()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        ensure!(
            self.max_affiliate_fee_bps <= 10_000u16,
            ContractError::InvalidAffiliateFee { max: 10_000u16 }
        );
        Ok(())
    }
}
