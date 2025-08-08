use std::collections::HashSet;

use crate::ContractError;
use cosmwasm_std::{
    coins, ensure, Addr, Api, BankMsg, CosmosMsg, Decimal, Order, QuerierWrapper, Storage, Uint128,
};
use cw_storage_plus::Map;
use nami_rs::asset_allocation::AssetAllocation;
use nami_rs::index_nav::AllocationResponse;
use nami_rs::{index_nav::VaultStatusResponse, OracleConfig};
use rujira_rs::{fin, Oracle};

static ALLOCATIONS: Map<&str, AssetAllocation<OracleConfig>> = Map::new("allocation");

pub struct Vault<'a> {
    pub api: &'a dyn Api,
    pub querier: &'a QuerierWrapper<'a>,
    pub address: Addr,
    pub quote_denom: String,
}

impl<'a> Vault<'a> {
    pub fn new(
        api: &'a dyn Api,
        querier: &'a QuerierWrapper,
        address: Addr,
        quote_denom: String,
    ) -> Self {
        Self {
            api,
            querier,
            address,
            quote_denom,
        }
    }

    pub fn init(
        &self,
        storage: &mut dyn Storage,
        allocations: Vec<AssetAllocation<OracleConfig>>,
    ) -> Result<(), ContractError> {
        ensure!(
            allocations.iter().map(|a| a.weight).sum::<Decimal>() == Decimal::one(),
            ContractError::WeightOne
        );

        let quote_count = allocations
            .iter()
            .filter(|a| a.swap_contract.is_none())
            .count();
        ensure!(quote_count == 1, ContractError::MissingQuoteAllocation);

        allocations
            .into_iter()
            .try_for_each(|alloc| self.save_allocation(storage, alloc))?;

        Ok(())
    }

    pub fn update_allocations(
        &self,
        storage: &mut dyn Storage,
        target_allocations: Vec<AssetAllocation<OracleConfig>>,
    ) -> Result<(), ContractError> {
        // 1) Validate overall weight == 1 and exactly one quote allocation
        let total_weight: Decimal = target_allocations.iter().map(|a| a.weight).sum();
        ensure!(total_weight == Decimal::one(), ContractError::WeightOne);

        // Find the single quote-denom allocation
        let mut quote_iters = target_allocations
            .iter()
            .filter(|a| a.swap_contract.is_none());
        let quote_alloc = quote_iters
            .next()
            .ok_or(ContractError::MissingQuoteAllocation)?;
        ensure!(
            quote_iters.next().is_none(),
            ContractError::MissingQuoteAllocation
        );

        // 2) Load current allocations from storage
        let (curr_quote, curr_others) = self.load_allocations(storage)?;
        // Ensure the quote denom hasn't changed
        ensure!(
            curr_quote.denom == quote_alloc.denom,
            ContractError::MissingQuoteAllocation
        );

        // 3) Remove any existing allocations NOT in the new set
        //    Build a set of target denoms for fast lookup
        let target_denoms: HashSet<&str> = target_allocations
            .iter()
            .map(|a| a.denom.as_str())
            .collect();

        for alloc in curr_others {
            if !target_denoms.contains(alloc.denom.as_str()) {
                // Only remove if it's not in the new set and the balance is zero
                self.remove_allocation(storage, alloc.denom)?;
            }
        }

        // 4) Save or overwrite all target allocations (quote + others)
        //    Overwrite is safe because we've already removed any allocations NOT in the new set
        for alloc in target_allocations {
            self.save_allocation(storage, alloc)?;
        }

        Ok(())
    }

    pub fn save_allocation(
        &self,
        storage: &mut dyn Storage,
        allocation: AssetAllocation<OracleConfig>,
    ) -> Result<(), ContractError> {
        ensure!(
            allocation.swap_contract.is_some() || allocation.denom == self.quote_denom,
            ContractError::InvalidSwapContract
        );
        if let Some(ref swap_addr) = allocation.swap_contract {
            self.api.addr_validate(swap_addr)?;
            let cfg: fin::ConfigResponse = self
                .querier
                .query_wasm_smart(swap_addr, &fin::QueryMsg::Config {})?;
            let (quote, base) = (cfg.denoms.quote(), cfg.denoms.base());
            ensure!(
                // either (quote_denom, denom) == (quote, base)
                (quote == self.quote_denom && base == allocation.denom)
                // or flipped: (quote_denom, denom) == (base, quote)
                || (base == self.quote_denom && quote == allocation.denom),
                ContractError::InvalidDenomPair {}
            );
            allocation.oracle.price(*self.querier)?;
        }
        ensure!(
            allocation.slippage.gt(&Decimal::zero()) && allocation.slippage.lt(&Decimal::one()),
            ContractError::SlippageOne
        );
        ALLOCATIONS.save(storage, &allocation.denom, &allocation)?;
        Ok(())
    }

    pub fn load_allocations(
        &self,
        storage: &dyn Storage,
    ) -> Result<
        (
            AssetAllocation<OracleConfig>,
            Vec<AssetAllocation<OracleConfig>>,
        ),
        ContractError,
    > {
        let quote = ALLOCATIONS.load(storage, self.quote_denom.as_str())?;
        let others = ALLOCATIONS
            .range(storage, None, None, Order::Ascending)
            .try_fold(Vec::new(), |mut acc, item| -> Result<_, ContractError> {
                let (_, alloc) = item?;
                if alloc.denom != self.quote_denom {
                    acc.push(alloc);
                }
                Ok(acc)
            })?;

        Ok((quote, others))
    }

    pub fn quote_price(&self, storage: &dyn Storage) -> Result<Decimal, ContractError> {
        let quote_asset = ALLOCATIONS.load(storage, self.quote_denom.as_str())?;
        let (_, price, _) = quote_asset.snapshot(&self.address, self.querier)?;
        Ok(price)
    }

    pub fn nav(
        &self,
        storage: &dyn Storage,
        amount: Option<Uint128>,
        shares: Uint128,
    ) -> Result<Decimal, ContractError> {
        let (quote, others) = self.load_allocations(storage)?;
        let (q_bal, q_price, _) = quote.snapshot(&self.address, self.querier)?;
        let sub = amount.unwrap_or_default();
        let starting =
            Decimal::from_ratio(q_bal.checked_sub(sub)?, Uint128::one()).checked_mul(q_price)?;
        let total =
            others
                .iter()
                .try_fold(starting, |acc, alloc| -> Result<Decimal, ContractError> {
                    let (_, _, v) = alloc.snapshot(&self.address, self.querier)?;
                    Ok(acc + v)
                })?;

        if shares.is_zero() {
            return Ok(q_price);
        }
        let share_ratio = Decimal::from_ratio(shares, Uint128::one());
        Ok(total.checked_div(share_ratio)?)
    }

    pub fn rebalance(&self, storage: &dyn Storage) -> Result<Vec<CosmosMsg>, ContractError> {
        let (quote, others) = self.load_allocations(storage)?;
        let quote_snapshot = quote.snapshot(&self.address, self.querier)?;

        let total = others.iter().try_fold(
            quote_snapshot.2,
            |acc, alloc| -> Result<Decimal, ContractError> {
                Ok(acc + alloc.snapshot(&self.address, self.querier)?.2)
            },
        )?;

        let msgs = others
            .into_iter()
            .filter_map(|alloc| {
                alloc
                    .rebalance_msg(&self.address, self.querier, total, &quote, quote_snapshot)
                    .transpose() // Result<Option<Msg>, Err> -> Option<Result<Msg,Err>>
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(msgs)
    }

    pub fn withdraw(
        &self,
        storage: &dyn Storage,
        value: Uint128,
        sender: Addr,
        slippage: Option<Decimal>,
    ) -> Result<Vec<CosmosMsg>, ContractError> {
        let (quote, others) = self.load_allocations(storage)?;
        let (quote_bal, quote_price, _) = quote.snapshot(&self.address, self.querier)?;
        let slip = slippage.unwrap_or(Decimal::one());

        let amount = Decimal::from_ratio(value, Uint128::one())
            .checked_div(quote_price)?
            .to_uint_floor();

        let min_amount = Decimal::from_ratio(amount, Uint128::one())
            .checked_mul(Decimal::one().checked_sub(slip)?)
            .unwrap_or_default()
            .to_uint_floor();

        let send_quote = amount.min(quote_bal);
        let mut msgs = vec![BankMsg::Send {
            amount: coins(send_quote.u128(), &quote.denom),
            to_address: sender.to_string(),
        }
        .into()];

        // remaining must be in usd beacuse the swap_msg uses the oracle price in usd to calculate the swap amount
        let remaining = Decimal::from_ratio(amount.checked_sub(send_quote)?, Uint128::one())
            .checked_mul(quote_price)?
            .to_uint_floor();

        if remaining.is_zero() {
            ensure!(send_quote >= min_amount, ContractError::SlippageExceeded {});
            return Ok(msgs);
        }

        let total_weight = others.iter().map(|a| a.weight).sum::<Decimal>();
        // max_cost is the amount of quote that can be spent to withdraw the remaining amount
        let max_cost = amount.checked_sub(min_amount)?;

        let (swap_msgs, _) = others.into_iter().try_fold(
            (Vec::new(), remaining),
            |(mut msgs, rem), alloc| -> Result<(Vec<CosmosMsg>, Uint128), ContractError> {
                let share = alloc.weight.checked_div(total_weight)?;
                let amt = Decimal::from_ratio(rem, Uint128::one())
                    .checked_mul(share)?
                    .to_uint_floor();
                if amt.is_zero() {
                    return Ok((msgs, rem));
                }

                // convert the amt in quote to build the correct min_return
                let amt_in_quote = Decimal::from_ratio(amt, Uint128::one())
                    .checked_div(quote_price)?
                    .to_uint_floor();

                let alloc_max = Decimal::from_ratio(max_cost, Uint128::one())
                    .checked_mul(share)?
                    .to_uint_floor();

                // min return should be in quote because fin needs it in the receive denom
                let min_return = (amt_in_quote > alloc_max)
                    .then(|| amt_in_quote.checked_sub(alloc_max).unwrap());

                msgs.push(alloc.swap_msg(&self.address, amt, &sender, self.querier, min_return)?);

                let rem = rem.checked_sub(amt)?;
                Ok((msgs, rem))
            },
        )?;
        msgs.extend(swap_msgs);

        Ok(msgs)
    }

    pub fn status(
        &self,
        storage: &dyn Storage,
        shares: Uint128,
    ) -> Result<VaultStatusResponse, ContractError> {
        let nav_per_share = self.nav(storage, None, shares)?;
        let nav = nav_per_share
            .checked_mul(Decimal::from_ratio(shares, Uint128::one()))?
            .to_uint_floor();

        let (base, others) = self.load_allocations(storage)?;
        let (_, quote_price, _) = base.snapshot(&self.address, self.querier)?;
        let redemption_rate = nav_per_share.checked_div(quote_price)?;

        let allocation = others
            .into_iter()
            .chain(std::iter::once(base.clone()))
            .map(|alloc| -> Result<AllocationResponse, ContractError> {
                let (bal, price, _val) = alloc.snapshot(&self.address, self.querier)?;
                Ok(AllocationResponse {
                    denom: alloc.denom,
                    swap_contract: alloc.swap_contract,
                    balance: bal,
                    price,
                    weight: alloc.weight,
                    threshold: alloc.threshold,
                    slippage: alloc.slippage,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(VaultStatusResponse {
            nav,
            shares,
            nav_per_share,
            redemption_rate,
            allocation,
        })
    }

    pub fn remove_allocation(
        &self,
        storage: &mut dyn Storage,
        denom: String,
    ) -> Result<(), ContractError> {
        let coin = self.querier.query_balance(&self.address, denom.clone())?;
        ensure!(coin.amount.is_zero(), ContractError::WeightNotZero);
        ALLOCATIONS.remove(storage, denom.as_str());
        Ok(())
    }
}
