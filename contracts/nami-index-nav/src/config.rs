use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, StdResult, Storage};
use cw_storage_plus::Item;
use nami_rs::index_nav::{ConfigResponse, InstantiateMsg};

use crate::ContractError;

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub quote_denom: String,
    pub fee_collector: Addr,
}

impl Config {
    pub fn new(api: &dyn Api, msg: InstantiateMsg) -> StdResult<Self> {
        Ok(Self {
            quote_denom: msg.quote_denom,
            fee_collector: api.addr_validate(&msg.fee_collector)?,
        })
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }

    pub fn update(
        &mut self,
        api: &dyn Api,
        fee_collector: Option<String>,
    ) -> Result<(), ContractError> {
        if let Some(fee_collector) = fee_collector {
            self.fee_collector = api.addr_validate(&fee_collector)?;
        }
        Ok(())
    }
}

impl From<Config> for ConfigResponse {
    fn from(config: Config) -> Self {
        Self {
            quote_denom: config.quote_denom,
            fee_collector: config.fee_collector.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{MockApi, MockStorage};

    #[test]
    fn test_config_full_flow() {
        let api = MockApi::default();
        let collector = api.addr_make("collector");

        let mut cfg = Config {
            quote_denom: "uusd".to_string(),
            fee_collector: collector.clone(),
        };

        let mut storage = MockStorage::new();
        cfg.save(&mut storage).unwrap();
        let loaded = Config::load(&storage).unwrap();
        assert_eq!(loaded.quote_denom, cfg.quote_denom);
        assert_eq!(loaded.fee_collector, cfg.fee_collector);

        cfg.update(&api, None).unwrap();

        let new_collector = api.addr_make("new_coll");
        cfg.update(&api, Some(new_collector.to_string())).unwrap();

        cfg.update(&api, Some("".to_string())).unwrap_err();

        let resp: ConfigResponse = cfg.into();
        assert_eq!(resp.quote_denom, "uusd");
        assert_eq!(resp.fee_collector, new_collector.to_string());
    }
}
