use crate::ContractError;
use cosmwasm_std::{
    ensure, to_json_binary, Addr, Api, Coin, CosmosMsg, Decimal, Order, QuerierWrapper, Storage,
    Uint128, WasmMsg,
};
use cw_storage_plus::Map;
use nami_rs::index_fixed::{CallbackType, VaultStatusResponse};
use rujira_rs::fin::{self, SwapRequest};
use std::collections::HashMap;

static ALLOCATIONS: Map<&str, (Uint128, Addr)> = Map::new("allocations");

pub struct Vault<'a> {
    pub api: &'a dyn Api,
    pub querier: &'a QuerierWrapper<'a>,
    pub address: Addr,
}

impl<'a> Vault<'a> {
    pub fn new(api: &'a dyn Api, querier: &'a QuerierWrapper, address: Addr) -> Self {
        Self {
            api,
            querier,
            address,
        }
    }
    pub fn init(
        &self,
        storage: &mut dyn Storage,
        allocation: Vec<(String, Uint128, String)>,
        quote_denom: &str,
    ) -> Result<(), ContractError> {
        for (denom, weight, contract) in allocation {
            self.add_allocation(storage, quote_denom, &denom, weight, &contract)?;
        }
        Ok(())
    }

    pub fn add_allocation(
        &self,
        storage: &mut dyn Storage,
        quote_denom: &str,
        denom: &str,
        weight: Uint128,
        contract: &str,
    ) -> Result<(), ContractError> {
        let contract = self.api.addr_validate(contract)?;
        let config: fin::ConfigResponse = self
            .querier
            .query_wasm_smart(&contract, &fin::QueryMsg::Config {})?;
        ensure!(
            config.denoms.quote() == quote_denom || config.denoms.base() == quote_denom,
            ContractError::InvalidQuoteDenom
        );
        ALLOCATIONS.save(storage, denom, &(weight, contract))?;
        Ok(())
    }

    pub fn deposit(storage: &dyn Storage, funds: Vec<Coin>) -> Result<Uint128, ContractError> {
        let fund_map: HashMap<_, _> = funds
            .into_iter()
            .map(|c| (c.denom.clone(), c.amount))
            .collect();

        let mut share: Option<Uint128> = None;

        for item in ALLOCATIONS.range(storage, None, None, Order::Ascending) {
            let (denom, (required, _)) = item?;
            let received = fund_map
                .get(&denom)
                .ok_or(ContractError::MissingDenomination(denom.clone()))?;
            let this_share = Decimal::from_ratio(*received, required).to_uint_floor();
            match share {
                None => share = Some(this_share),
                Some(prev) if prev == this_share => {}
                _ => return Err(ContractError::DepositProportionsNotIdentical),
            }
        }

        Ok(share.unwrap_or_default())
    }

    pub fn withdraw(
        storage: &mut dyn Storage,
        shares: Uint128,
    ) -> Result<Vec<Coin>, ContractError> {
        if shares.is_zero() {
            return Ok(vec![]);
        }
        ALLOCATIONS
            .range(storage, None, None, Order::Ascending)
            .map(|item| {
                let (denom, (amount_per_share, _)) = item?;
                let total_amount = amount_per_share.checked_mul(shares)?;
                Ok(Coin::new(total_amount, denom))
            })
            .collect()
    }

    pub fn rebalance(
        &self,
        storage: &mut dyn Storage,
        total_shares: Uint128,
    ) -> Result<(), ContractError> {
        if total_shares.is_zero() {
            return Ok(());
        }

        let entries: Vec<(String, Addr)> = ALLOCATIONS
            .range(storage, None, None, Order::Ascending)
            .map(|item| {
                let (denom, (_, contract)) = item?;
                Ok((denom, contract))
            })
            .collect::<Result<_, ContractError>>()?;

        for (denom, contract) in entries {
            let coin = self
                .querier
                .query_balance(self.address.clone(), denom.clone())?;
            let amount_per_share = Decimal::from_ratio(coin.amount, total_shares).to_uint_floor();
            ALLOCATIONS.save(storage, &denom, &(amount_per_share, contract))?;
        }

        Ok(())
    }

    pub fn reallocate(
        storage: &mut dyn Storage,
        from: &str,
        to: &str,
        weight: Uint128,
        total_shares: Uint128,
        min_return: Option<Uint128>,
    ) -> Result<CosmosMsg, ContractError> {
        let (curr_weight, swap_from) = ALLOCATIONS.load(storage, from)?;
        let (_, swap_to) = ALLOCATIONS.load(storage, to)?;

        ensure!(curr_weight.gt(&weight), ContractError::InvalidWeight);

        let amount_to_swap = curr_weight.checked_sub(weight)?.checked_mul(total_shares)?;

        let callback = to_json_binary(&CallbackType::AfterReallocate {
            swap_to,
            min_return,
        })?;

        Ok(WasmMsg::Execute {
            contract_addr: swap_from.to_string(),
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                min_return,
                to: None,
                callback: Some(callback.into()),
            }))?,
            funds: vec![Coin::new(amount_to_swap, from)],
        }
        .into())
    }

    pub fn remove_allocation(
        storage: &mut dyn Storage,
        denom: String,
    ) -> Result<(), ContractError> {
        let (weight, _) = ALLOCATIONS.load(storage, &denom)?;
        ensure!(weight.is_zero(), ContractError::WeightNotZero);
        ALLOCATIONS.remove(storage, &denom);
        Ok(())
    }

    pub fn status(
        storage: &dyn Storage,
        total_shares: Uint128,
    ) -> Result<VaultStatusResponse, ContractError> {
        let allocation = ALLOCATIONS
            .range(storage, None, None, Order::Ascending)
            .map(|res| res.map(|(denom, (weight, _))| (denom, weight)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(VaultStatusResponse {
            total_shares,
            allocation,
        })
    }

    pub fn is_auth(storage: &dyn Storage, sender: &Addr) -> Result<bool, ContractError> {
        ALLOCATIONS
            .range(storage, None, None, Order::Ascending)
            .try_fold(false, |found, item| {
                let (_, (_, contract)) = item?;
                Ok(found || contract == *sender)
            })
    }
}
