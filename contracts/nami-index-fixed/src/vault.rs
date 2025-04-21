use std::collections::HashMap;

use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coins, ensure, to_json_binary, Addr, Api, Coin, CosmosMsg, Decimal, Env, QuerierWrapper,
    Storage, Uint128, WasmMsg,
};
use cw_storage_plus::Map;
use cw_utils::PaymentError;
use nami_rs::index_fixed::CallbackType;
use rujira_rs::fin::{self, SwapRequest};

static ALLOCATION: Map<&str, (Uint128, Addr)> = Map::new("allocation");

#[cw_serde]
pub struct Vault {}

impl Vault {
    pub fn init(
        storage: &mut dyn Storage,
        api: &dyn Api,
        allocation: Vec<(String, Uint128, String)>,
    ) -> Result<(), ContractError> {
        for (denom, amount, contract) in allocation {
            ALLOCATION.save(storage, &denom, &(amount, api.addr_validate(&contract)?))?;
        }
        Ok(())
    }

    pub fn deposit(storage: &mut dyn Storage, funds: Vec<Coin>) -> Result<Uint128, ContractError> {
        let fund_map: HashMap<_, _> = funds
            .into_iter()
            .map(|coin| (coin.denom, coin.amount))
            .collect();

        let mut allocations = ALLOCATION.keys(storage, None, None, cosmwasm_std::Order::Ascending);

        let mut shares: Option<Uint128> = None;

        while let Some(denom) = allocations.next() {
            let denom = denom?;
            let (required_amount, _) = ALLOCATION.load(storage, &denom)?;

            let received_amount = fund_map
                .get(&denom)
                .ok_or_else(|| ContractError::Payment(PaymentError::MissingDenom(denom.clone())))?;

            ensure!(
                !received_amount.is_zero(),
                ContractError::Invalid(format!("Invalid deposit amount for {}", denom))
            );

            let units = Decimal::from_ratio(*received_amount, required_amount).to_uint_floor();

            match shares {
                Some(ref s) if s != &units => {
                    return Err(ContractError::Invalid(
                        "Deposit proportions do not match allocation".to_string(),
                    ));
                }
                None => shares = Some(units),
                _ => {}
            }
        }

        Ok(shares.unwrap_or_else(Uint128::zero))
    }

    pub fn withdraw(
        storage: &mut dyn Storage,
        shares: Uint128,
    ) -> Result<Vec<Coin>, ContractError> {
        ensure!(
            !shares.is_zero(),
            ContractError::Invalid("Shares must be greater than zero".to_string())
        );

        let mut coins: Vec<Coin> = vec![];

        let mut allocations = ALLOCATION.keys(storage, None, None, cosmwasm_std::Order::Ascending);

        while let Some(denom) = allocations.next() {
            let denom = denom?;
            let (amount_per_share, _) = ALLOCATION.load(storage, &denom)?;
            let total_amount = amount_per_share
                .checked_mul(shares)
                .map_err(|_| ContractError::Invalid("Overflow during withdrawal".to_string()))?;

            coins.push(Coin {
                denom,
                amount: total_amount,
            });
        }

        Ok(coins)
    }

    pub fn rebalance(
        storage: &mut dyn Storage,
        env: &Env,
        querier: &QuerierWrapper,
        total_shares: Uint128,
    ) -> Result<(), ContractError> {
        let denoms: Vec<String> = ALLOCATION
            .keys(storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<Result<_, _>>()?;

        for denom in denoms {
            let (_, contract) = ALLOCATION.load(storage, &denom)?;

            let balance = Self::balance(querier, denom.clone(), env.contract.address.clone())?;

            let amount_per_share = Decimal::from_ratio(balance, total_shares).to_uint_floor();

            ALLOCATION.save(storage, &denom, &(amount_per_share, contract))?;
        }

        Ok(())
    }

    pub fn balance(
        querier: &QuerierWrapper,
        denom: String,
        address: Addr,
    ) -> Result<Uint128, ContractError> {
        let coin = querier.query_balance(address, denom)?;
        Ok(coin.amount)
    }

    pub fn reallocate(
        storage: &mut dyn Storage,
        from: String,
        to: String,
        weight: Uint128,
        total_shares: Uint128,
    ) -> Result<CosmosMsg, ContractError> {
        ensure!(
            ALLOCATION.has(storage, &from),
            ContractError::Invalid("Invalid from".to_string())
        );
        ensure!(
            ALLOCATION.has(storage, &to),
            ContractError::Invalid("Invalid to".to_string())
        );

        let (curr_weight, swap_from) = ALLOCATION.load(storage, &from)?;
        let (_, swap_to) = ALLOCATION.load(storage, &to)?;

        ensure!(
            curr_weight.gt(&weight),
            ContractError::Invalid(
                "Invalid weight must be greater than current weight".to_string()
            )
        );

        let amount_to_swap = curr_weight.checked_sub(weight)?.checked_mul(total_shares)?;

        let callback = to_json_binary(&CallbackType::AfterReallocate { swap_to })?;

        Ok(WasmMsg::Execute {
            contract_addr: swap_from.to_string(),
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                min_return: None,
                to: None,
                callback: Some(callback.into()),
            }))?,
            funds: coins(amount_to_swap.into(), from),
        }
        .into())
    }

    pub fn after_reallocate(
        env: &Env,
        querier: QuerierWrapper,
        swap_to: Addr,
        base_denom: String,
    ) -> Result<CosmosMsg, ContractError> {
        let coin = querier.query_balance(env.contract.address.to_string(), base_denom.clone())?;

        Ok(WasmMsg::Execute {
            contract_addr: swap_to.to_string(),
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                min_return: None,
                to: None,
                callback: None,
            }))?,
            funds: vec![coin],
        }
        .into())
    }
}
