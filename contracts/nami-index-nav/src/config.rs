use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Decimal, QuerierWrapper, StdResult, Storage};
use cw_storage_plus::Item;
use nami_rs::index_nav::InstantiateMsg;
use nami_rs::OracleConfig;
use rujira_rs::Oracle;
use std::collections::HashSet;

use crate::{asset_allocation::AssetAllocation, ContractError};

static CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Config {
    pub base_denom: String,
    pub fee_collector: Addr,
    pub allocations: Vec<AssetAllocation<OracleConfig>>,
}

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
                        swap_contract: if c.is_empty() {
                            None
                        } else {
                            Some(api.addr_validate(c)?)
                        },
                        oracle: d.clone(),
                    })
                })
                .collect::<StdResult<Vec<AssetAllocation<OracleConfig>>>>()?,
        })
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG.load(storage)
    }

    pub fn validate(&self, querier: QuerierWrapper) -> Result<(), ContractError> {
        if self.base_denom.is_empty() {
            return Err(ContractError::Invalid("Empty base_denom".to_string()));
        }
        if !self.allocations.iter().any(|a| a.denom == self.base_denom) {
            return Err(ContractError::Invalid(
                "base_denom must be in allocations".to_string(),
            ));
        }
        let mut denoms = HashSet::new();
        let mut base_denom_found = false;
        for alloc in &self.allocations {
            if alloc.denom.is_empty() {
                return Err(ContractError::Invalid("Empty denom".to_string()));
            }
            if !denoms.insert(&alloc.denom) {
                return Err(ContractError::Invalid(format!(
                    "Duplicate denom: {}",
                    alloc.denom
                )));
            }
            if alloc.denom == self.base_denom {
                base_denom_found = true;
            }
            if alloc.weight.is_zero() {
                return Err(ContractError::Invalid(format!(
                    "Zero weight for denom: {}",
                    alloc.denom
                )));
            }
            if alloc.swap_contract.is_none() && alloc.denom != self.base_denom {
                return Err(ContractError::Invalid(
                    "Swap contract needed for non base denom".to_string(),
                ));
            }
            alloc.oracle.price(querier)?;
        }
        if !base_denom_found {
            return Err(ContractError::Invalid(
                "base_denom must be in allocations".to_string(),
            ));
        }
        let weight_sum: Decimal = self.allocations.iter().map(|a| a.weight).sum();
        if weight_sum != Decimal::one() {
            return Err(ContractError::Invalid(format!(
                "Weights must sum to 100%, got {}",
                weight_sum
            )));
        }
        Ok(())
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG.save(storage, self)
    }

    pub fn update(
        &mut self,
        base_denom: Option<String>,
        allocations: Option<Vec<AssetAllocation<OracleConfig>>>,
        fee_collector: Option<Addr>,
    ) -> StdResult<()> {
        if let Some(base_denom) = base_denom {
            self.base_denom = base_denom;
        }
        if let Some(allocations) = allocations {
            self.allocations = allocations;
        }
        if let Some(fee_collector) = fee_collector {
            self.fee_collector = fee_collector;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        testing::{mock_dependencies, MockApi},
        Decimal,
    };
    use nami_rs::{index_nav::InstantiateMsg, FeeRates, OracleConfig};
    use rujira_rs::{Chain, Layer1Asset, TokenMetadata};

    fn valid_instantiate_msg() -> InstantiateMsg {
        let app = MockApi::default();
        let fee_collector = app.addr_make("fee_collector");
        let swap_contract = app.addr_make("swap_contract");

        InstantiateMsg {
            receipt: TokenMetadata {
                name: "token".to_string(),
                symbol: "token".to_string(),
                description: "token description".to_string(),
                display: "token display".to_string(),
                uri: None,
                uri_hash: None,
            },
            base_denom: "usdc.ETH".to_string(),
            fee_collector: fee_collector.to_string(),
            fees: FeeRates {
                management: None,
                performance: None,
                transaction: None,
            },
            target_denoms: vec![
                (
                    "btc".to_string(),
                    Decimal::percent(25),
                    swap_contract.to_string(),
                    OracleConfig::Layer1(Layer1Asset::new(Chain::Btc, "BTC")),
                ),
                (
                    "eth".to_string(),
                    Decimal::percent(25),
                    swap_contract.to_string(),
                    OracleConfig::Layer1(Layer1Asset::new(Chain::Eth, "ETH")),
                ),
                (
                    "usdc.ETH".to_string(),
                    Decimal::percent(50),
                    swap_contract.to_string(),
                    OracleConfig::Layer1(Layer1Asset::new(Chain::Eth, "usdc.ETH")),
                ),
            ],
        }
    }

    #[test]
    fn test_config_new_and_update() {
        let deps = mock_dependencies();
        let msg = valid_instantiate_msg();

        let mut config = Config::new(&deps.api, msg).unwrap();
        let app = MockApi::default();
        let fee_collector = app.addr_make("fee_collector");
        let swap_contract = app.addr_make("swap_contract");

        assert_eq!(config.base_denom, "usdc.ETH");
        assert_eq!(config.fee_collector, fee_collector);
        assert_eq!(config.allocations.len(), 3);
        assert_eq!(config.allocations[0].denom, "btc");
        assert_eq!(config.allocations[0].weight, Decimal::percent(25));
        assert_eq!(
            config.allocations[0].swap_contract,
            Some(swap_contract.clone())
        );

        let new_fee_collector = app.addr_make("new_fee_collector");
        let new_allocations = vec![
            AssetAllocation::new(
                "avax".to_string(),
                Decimal::percent(20),
                Some(swap_contract.clone()),
                OracleConfig::Layer1(Layer1Asset::new(Chain::Avax, "AVAX")),
            ),
            AssetAllocation::new(
                "doge".to_string(),
                Decimal::percent(80),
                Some(swap_contract.clone()),
                OracleConfig::Layer1(Layer1Asset::new(Chain::Doge, "DOGE")),
            ),
        ];
        config
            .update(
                Some("usdc.BTC".to_string()),
                Some(new_allocations),
                Some(new_fee_collector.clone()),
            )
            .unwrap();

        // Assert the updated values
        assert_eq!(config.base_denom, "usdc.BTC");
        assert_eq!(config.fee_collector, new_fee_collector);
        assert_eq!(config.allocations.len(), 2);
        assert_eq!(config.allocations[0].denom, "avax");
        assert_eq!(config.allocations[1].denom, "doge");
    }
}
