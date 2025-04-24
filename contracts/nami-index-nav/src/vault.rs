use crate::{asset_allocation::AssetAllocation, config::Config, ContractError};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coins, Addr, BankMsg, CosmosMsg, Decimal, Env, QuerierWrapper, Uint128};
use nami_rs::OracleConfig;

#[cw_serde]
pub struct Vault {
    pub address: Addr,
    pub allocations: Vec<AssetAllocation<OracleConfig>>,
}

impl Vault {
    pub fn new(env: &Env, config: &Config) -> Result<Self, ContractError> {
        Ok(Vault {
            address: env.contract.address.clone(),
            allocations: config.allocations.clone(),
        })
    }

    pub fn nav(
        &self,
        querier: &QuerierWrapper,
        amount: Option<Uint128>,
        shares: Uint128,
    ) -> Result<Decimal, ContractError> {
        let mut total_value = Decimal::zero();
        for alloc in &self.allocations {
            if alloc.swap_contract.is_none() && amount.is_some() {
                let amount = amount.unwrap();
                let (balance, price) = alloc.quote(&self.address, querier)?;
                total_value += Decimal::from_ratio(balance.checked_sub(amount)?, Uint128::one())
                    .checked_mul(price)?;
            } else {
                total_value += alloc.value(&self.address, querier)?;
            }
        }
        let nav = if shares.is_zero() {
            Decimal::one()
        } else {
            total_value.checked_div(Decimal::from_ratio(shares, Uint128::one()))?
        };
        Ok(nav)
    }

    pub fn rebalance(&self, querier: &QuerierWrapper) -> Result<Vec<CosmosMsg>, ContractError> {
        let mut total_value = Decimal::zero();
        for alloc in &self.allocations {
            total_value += alloc.value(&self.address, querier)?;
        }
        let base_alloc = self.base();
        let (base_balance, _) = base_alloc.quote(&self.address, querier)?;

        let mut msgs = Vec::new();
        for alloc in self
            .allocations
            .iter()
            .filter(|a| a.swap_contract.is_some())
        {
            if let Some(msg) = alloc.rebalance_msg(
                &self.address,
                querier,
                total_value,
                &base_alloc,
                base_balance,
            )? {
                msgs.push(msg);
            }
        }
        Ok(msgs)
    }

    pub fn base(&self) -> AssetAllocation<OracleConfig> {
        self.allocations
            .iter()
            .find(|a| a.swap_contract.is_none())
            .cloned()
            .unwrap()
    }

    pub fn withdraw(
        &self,
        querier: &QuerierWrapper,
        amount: Uint128,
        sender: Addr,
    ) -> Result<Vec<CosmosMsg>, ContractError> {
        let base_alloc = self.base();
        let (base_bal, _) = base_alloc.quote(&self.address, querier)?;
        let mut msgs = Vec::new();

        let send_base = amount.min(base_bal);
        msgs.push(
            BankMsg::Send {
                amount: coins(send_base.u128(), &base_alloc.denom),
                to_address: sender.to_string(),
            }
            .into(),
        );
        let remaining = amount.checked_sub(send_base)?;
        if remaining.gt(&Uint128::zero()) {
            msgs.extend(self.pro_rata_withdraw(remaining, querier, sender)?);
        }
        Ok(msgs)
    }

    fn pro_rata_withdraw(
        &self,
        amount: Uint128,
        querier: &QuerierWrapper,
        sender: Addr,
    ) -> Result<Vec<CosmosMsg>, ContractError> {
        let mut msgs = vec![];
        let mut remaining = amount;
        let total_weight = self
            .allocations
            .iter()
            .filter(|a| a.swap_contract.is_some())
            .map(|a| a.weight)
            .sum();

        for alloc in self
            .allocations
            .iter()
            .filter(|a| a.swap_contract.is_some())
        {
            let ratio = alloc.weight.checked_div(total_weight)?;
            let amount = Decimal::from_ratio(remaining, Uint128::one())
                .checked_mul(ratio)?
                .to_uint_floor();
            if amount.is_zero() {
                continue;
            }
            let msg = alloc.swap_msg(&self.address, amount, &sender, querier)?;
            msgs.push(msg);
            remaining = remaining.checked_sub(amount)?;
        }
        Ok(msgs)
    }
}
