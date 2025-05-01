use cosmwasm_std::{
    coins, ensure_eq, to_json_binary, CosmosMsg, Order, QuerierWrapper, StdResult, Storage, WasmMsg,
};
use cw_storage_plus::Map;
use nami_rs::index_entry_adapter::SwapEntry;
use rujira_rs::fin::{self, SwapRequest};

use crate::ContractError;

pub struct Status<'a> {
    pub swap_contracts: Map<&'a str, String>,
}

impl<'a> Status<'a> {
    pub const fn new() -> Self {
        Self {
            swap_contracts: Map::new("s/c"),
        }
    }

    pub fn add_contract(
        &self,
        storage: &mut dyn Storage,
        querier: &QuerierWrapper,
        key: &'a str,
        value: String,
        quote_denom: String,
    ) -> Result<(), ContractError> {
        let config: fin::ConfigResponse =
            querier.query_wasm_smart(&value, &fin::QueryMsg::Config {})?;
        ensure_eq!(
            config.denoms.quote(),
            quote_denom,
            ContractError::Invalid("Invalid quote denom".to_string())
        );
        self.swap_contracts.save(storage, key, &value)?;
        Ok(())
    }

    pub fn remove_contract(&self, storage: &mut dyn Storage, key: &'a str) -> StdResult<()> {
        self.swap_contracts.remove(storage, key);
        Ok(())
    }

    pub fn swap_msg(
        &self,
        storage: &dyn Storage,
        key: &'a str,
        swap: SwapEntry,
        to: Option<String>,
    ) -> Result<CosmosMsg, ContractError> {
        let contract_addr = self.swap_contracts.load(storage, key);
        match contract_addr {
            Ok(contract_addr) => Ok(WasmMsg::Execute {
                contract_addr,
                msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                    to,
                    min_return: swap.min_return,
                    callback: None,
                }))?,
                funds: coins(swap.amount.u128(), swap.denom),
            }
            .into()),
            Err(_) => Err(ContractError::SwapContractNotFound(key.to_string())),
        }
    }

    pub fn get_swap_contracts(&self, storage: &dyn Storage) -> StdResult<Vec<(String, String)>> {
        self.swap_contracts
            .range(storage, None, None, Order::Ascending)
            .map(|item| {
                let (key, value) = item?;
                Ok((key.to_string(), value))
            })
            .collect()
    }

    pub fn get(&self, storage: &dyn Storage, key: &'a str) -> StdResult<String> {
        self.swap_contracts.load(storage, key)
    }
}
