use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coins, ensure, to_json_binary, Addr, CheckedFromRatioError, CosmosMsg, Decimal, OverflowError,
    QuerierWrapper, StdError, Uint128, WasmMsg,
};
use rujira_rs::{
    fin::{self, SwapRequest},
    Oracle, OracleError,
};
use thiserror::Error;

#[cw_serde]
pub struct AssetAllocation<T: Oracle> {
    pub denom: String,
    pub weight: Decimal,
    pub swap_contract: Option<String>,
    pub oracle: T,
    pub threshold: Decimal,
    pub slippage: Decimal,
}

impl<T: Oracle> AssetAllocation<T> {
    pub fn new(
        denom: String,
        weight: Decimal,
        swap_contract: Option<String>,
        oracle: T,
        threshold: Decimal,
        slippage: Decimal,
    ) -> Self {
        Self {
            denom,
            weight,
            swap_contract,
            oracle,
            threshold,
            slippage,
        }
    }

    pub fn snapshot(
        &self,
        address: &Addr,
        querier: &QuerierWrapper,
    ) -> Result<(Uint128, Decimal, Decimal), AssetAllocationError> {
        let coin = querier.query_balance(address.clone(), self.denom.clone())?;
        let price = self.oracle.price(*querier)?;
        ensure!(!price.is_zero(), AssetAllocationError::ZeroPrice {});
        let val = Decimal::from_ratio(coin.amount, Uint128::one()).checked_mul(price)?;
        Ok((coin.amount, price, val))
    }

    pub fn target_value(&self, total_value: Decimal) -> Result<Decimal, AssetAllocationError> {
        Ok(total_value.checked_mul(self.weight)?)
    }

    pub fn rebalance_msg(
        &self,
        address: &Addr,
        querier: &QuerierWrapper,
        total_value: Decimal,
        quote_entry: &AssetAllocation<T>,
        quote_snapshot: (Uint128, Decimal, Decimal),
    ) -> Result<Option<CosmosMsg>, AssetAllocationError> {
        let (bal, price, curr_val) = self.snapshot(address, querier)?;

        let tgt_val = self.target_value(total_value)?;
        let diff = if curr_val > tgt_val {
            curr_val - tgt_val
        } else {
            tgt_val - curr_val
        };
        if diff <= tgt_val.checked_mul(self.threshold)? {
            return Ok(None);
        }

        let (delta, unit_price, slip_price, denom, available) = if curr_val > tgt_val {
            (
                curr_val - tgt_val,
                price,
                quote_snapshot.1,
                &self.denom,
                bal,
            )
        } else {
            (
                tgt_val - curr_val,
                quote_snapshot.1,
                price,
                &quote_entry.denom,
                quote_snapshot.0,
            )
        };

        let units = delta.checked_div(unit_price)?.to_uint_floor();
        if units.is_zero() || available < units {
            return Ok(None);
        }
        let funds = coins(units.u128(), denom);

        let contract_addr = match &self.swap_contract {
            Some(addr) => addr.clone(),
            None => return Ok(None),
        };

        let min_return = Some(
            delta
                .checked_mul(Decimal::one().checked_sub(self.slippage)?)?
                .checked_div(slip_price)?
                .to_uint_floor(),
        );

        let msg = WasmMsg::Execute {
            contract_addr,
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                to: None,
                min_return,
                callback: None,
            }))?,
            funds,
        }
        .into();

        Ok(Some(msg))
    }

    pub fn swap_msg(
        &self,
        address: &Addr,
        amount: Uint128,
        sender: &Addr,
        querier: &QuerierWrapper,
        min_return: Option<Uint128>,
    ) -> Result<CosmosMsg, AssetAllocationError> {
        let (bal, price, _curr_val) = self.snapshot(address, querier)?;
        let sell_amt = Decimal::from_ratio(amount, Uint128::one())
            .checked_div(price)?
            .to_uint_floor();
        ensure!(
            sell_amt > Uint128::zero() && bal >= sell_amt,
            AssetAllocationError::InsufficientFunds {}
        );
        let swap_contract = if let Some(swap_contract) = &self.swap_contract {
            swap_contract.clone()
        } else {
            return Err(AssetAllocationError::NoSwapContract {});
        };

        Ok(WasmMsg::Execute {
            contract_addr: swap_contract.clone(),
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                to: Some(sender.to_string()),
                min_return,
                callback: None,
            }))?,
            funds: coins(sell_amt.u128(), &self.denom),
        }
        .into())
    }
}

#[derive(Error, Debug)]
pub enum AssetAllocationError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("{0}")]
    OracleError(#[from] OracleError),

    #[error("Zero oracle price")]
    ZeroPrice,

    #[error("{0}")]
    CheckedFromRatio(#[from] CheckedFromRatioError),

    #[error("No swap contract")]
    NoSwapContract,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{coins, Addr, Coin, Decimal, OwnedDeps, QuerierWrapper, Uint128, WasmMsg};
    use std::str::FromStr;

    #[derive(Clone)]
    struct DummyOracle(Decimal);
    impl Oracle for DummyOracle {
        fn price(&self, _q: QuerierWrapper) -> Result<Decimal, OracleError> {
            Ok(self.0)
        }
    }

    fn setup_deps() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
        let balances: &[(&str, &[Coin])] = &[(
            "contract",
            &[
                Coin::new(1_000_000u128, "usdc.ETH"),
                Coin::new(10u128, "btc"),
                Coin::new(20u128, "eth"),
            ],
        )];
        let querier = MockQuerier::new(balances);
        OwnedDeps {
            storage: MockStorage::new(),
            api: MockApi::default(),
            querier,
            custom_query_type: std::marker::PhantomData,
        }
    }

    #[test]
    fn parametric_rebalance_and_swap() {
        let deps = setup_deps();
        let q = QuerierWrapper::new(&deps.querier);
        let addr = Addr::unchecked("contract");
        let user = Addr::unchecked("user");

        // common base asset
        let base = AssetAllocation::new(
            "usdc.ETH".into(),
            Decimal::percent(50),
            Some("base_swap".into()),
            DummyOracle(Decimal::one()),
            Decimal::percent(0),
            Decimal::percent(1),
        );
        let total = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_snap = base.snapshot(&addr, &q).unwrap();

        // rebalance cases: (alloc, expected swap address or None)
        let rebalance_cases = vec![
            // sell btc (price high → excess)
            (
                AssetAllocation::new(
                    "btc".into(),
                    Decimal::percent(25),
                    Some("btc_swap".into()),
                    DummyOracle(Decimal::from_str("100000").unwrap()),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                Some("btc_swap"),
            ),
            // buy eth (price lower → need quote funds)
            (
                AssetAllocation::new(
                    "eth".into(),
                    Decimal::percent(25),
                    Some("eth_swap".into()),
                    DummyOracle(Decimal::from_str("2000").unwrap()),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                Some("eth_swap"),
            ),
        ];
        // zero price → none
        let zero_price = AssetAllocation::new(
            "atom".into(),
            Decimal::percent(50),
            Some("swap".into()),
            DummyOracle(Decimal::zero()),
            Decimal::percent(100),
            Decimal::percent(1),
        );
        let res = zero_price
            .rebalance_msg(&addr, &q, total, &base, base_snap)
            .unwrap_err();
        assert!(matches!(res, AssetAllocationError::ZeroPrice));

        for (alloc, expect) in rebalance_cases {
            let res = alloc
                .rebalance_msg(&addr, &q, total, &base, base_snap)
                .unwrap();
            match expect {
                None => assert!(res.is_none()),
                Some(expected_addr) => {
                    let msg = res.unwrap();
                    if let CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr,
                        funds,
                        ..
                    }) = msg
                    {
                        assert_eq!(contract_addr, expected_addr);
                        assert!(!funds.is_empty());
                    } else {
                        panic!("expected WasmMsg::Execute");
                    }
                }
            }
        }

        // swap_msg cases: (alloc, amount, expected error substring or None)
        let swap_cases = vec![
            // no swap contract
            (
                AssetAllocation::new(
                    "btc".into(),
                    Decimal::one(),
                    None,
                    DummyOracle(Decimal::one()),
                    Decimal::zero(),
                    Decimal::percent(1),
                ),
                Uint128::new(1),
                Some("No swap contract"),
            ),
            // insufficient funds
            (
                AssetAllocation::new(
                    "usdc.ETH".into(),
                    Decimal::one(),
                    Some("swap".into()),
                    DummyOracle(Decimal::one()),
                    Decimal::zero(),
                    Decimal::percent(1),
                ),
                Uint128::new(2_000_000),
                Some("Insufficient funds"),
            ),
            // successful swap
            (
                AssetAllocation::new(
                    "usdc.ETH".into(),
                    Decimal::one(),
                    Some("swap".into()),
                    DummyOracle(Decimal::one()),
                    Decimal::zero(),
                    Decimal::percent(1),
                ),
                Uint128::new(100),
                None,
            ),
        ];

        for (alloc, amt, err_case) in swap_cases {
            let outcome = alloc.swap_msg(&addr, amt, &user, &q, None);
            match err_case {
                Some(err_str) => {
                    let e = outcome.unwrap_err().to_string();
                    assert!(
                        e.contains(err_str),
                        "expected error `{}`, got `{}`",
                        err_str,
                        e
                    );
                }
                None => {
                    let msg = outcome.unwrap();
                    if let CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr,
                        funds,
                        ..
                    }) = msg
                    {
                        assert_eq!(contract_addr, "swap");
                        assert_eq!(funds, coins(amt.u128(), "usdc.ETH"));
                    } else {
                        panic!("expected WasmMsg::Execute");
                    }
                }
            }
        }
    }

    #[test]
    fn test_rebalance_edge_cases() {
        let deps = setup_deps();
        let q = QuerierWrapper::new(&deps.querier);
        let addr = Addr::unchecked("contract");
        let base = AssetAllocation::new(
            "usdc.ETH".into(),
            Decimal::percent(50),
            Some("base_swap".into()),
            DummyOracle(Decimal::one()),
            Decimal::percent(0),
            Decimal::percent(1),
        );
        let total = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_snap = base.snapshot(&addr, &q).unwrap();

        // 1. Threshold-skip: diff <= target * threshold
        let skip = AssetAllocation::new(
            "btc".into(),
            Decimal::percent(25),
            Some("swap".into()),
            DummyOracle(Decimal::from_str("100000").unwrap()),
            Decimal::percent(100),
            Decimal::percent(1),
        );
        assert!(skip
            .rebalance_msg(&addr, &q, total, &base, base_snap.clone())
            .unwrap()
            .is_none());

        // 2. No-swap-contract: valid diff but swap_contract=None
        let no_swap = AssetAllocation::new(
            "btc".into(),
            Decimal::percent(25),
            None,
            DummyOracle(Decimal::from_str("100000").unwrap()),
            Decimal::percent(0),
            Decimal::percent(1),
        );
        assert!(no_swap
            .rebalance_msg(&addr, &q, total, &base, base_snap.clone())
            .unwrap()
            .is_none());

        // 3. Insufficient quote balance on buy branch
        let buy_insuf = AssetAllocation::new(
            "eth".into(),
            Decimal::percent(95),
            Some("eth_swap".into()),
            DummyOracle(Decimal::from_str("2000").unwrap()),
            Decimal::percent(0),
            Decimal::percent(1),
        );
        assert!(buy_insuf
            .rebalance_msg(&addr, &q, total, &base, base_snap.clone())
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_swap_edge_cases() {
        let deps = setup_deps();
        let q = QuerierWrapper::new(&deps.querier);
        let addr = Addr::unchecked("contract");
        let user = Addr::unchecked("user");

        // 1. Zero amount (should be an error - insufficient funds)
        let alloc = AssetAllocation::new(
            "btc".into(),
            Decimal::one(),
            Some("swap".into()),
            DummyOracle(Decimal::one()),
            Decimal::zero(),
            Decimal::percent(1),
        );
        assert!(matches!(
            alloc
                .swap_msg(&addr, Uint128::zero(), &user, &q, None)
                .unwrap_err(),
            AssetAllocationError::InsufficientFunds
        ));
    }
}
