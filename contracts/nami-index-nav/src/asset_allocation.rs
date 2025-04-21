use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coins, ensure, to_json_binary, Addr, Coin, CosmosMsg, Decimal, Fraction, OverflowError,
    OverflowOperation, QuerierWrapper, Uint128, WasmMsg, StdError,
};
use rujira_rs::{Oracle, fin::{self, SwapRequest}};

use crate::ContractError;

#[cw_serde]
pub struct AssetAllocation<T: Oracle> {
    pub denom: String,
    pub weight: Decimal,
    pub swap_contract: Option<Addr>,
    pub oracle: T,
}

impl<T: Oracle> AssetAllocation<T> {
    pub fn new(
        denom: String,
        weight: Decimal,
        swap_contract: Option<Addr>,
        oracle: T,
    ) -> Self {
        Self {
            denom,
            weight,
            swap_contract,
            oracle,
        }
    }

    pub fn quote(
        &self,
        address: &Addr,
        querier: &QuerierWrapper,
    ) -> Result<(Uint128, Decimal), ContractError> {
        let coin = querier.query_balance(address.clone(), self.denom.clone())?;
        let price = self.oracle.price(*querier)?;
        Ok((coin.amount, price))
    }

    pub fn value(
        &self,
        address: &Addr,
        querier: &QuerierWrapper,
    ) -> Result<Decimal, ContractError> {
        let (bal, price) = self.quote(address, querier)?;
        let price_numerator = price.numerator();
        if price_numerator > Uint128::zero() {
            let max_safe_bal = Uint128::new(u128::MAX / price_numerator.u128());
            if bal > max_safe_bal {
                return Err(ContractError::Overflow(OverflowError {
                    operation: OverflowOperation::Mul,
                }));
            }
        }
        let bal_decimal = Decimal::from_ratio(bal, Uint128::one());
        Ok(bal_decimal.checked_mul(price)?)
    }

    pub fn target_value(&self, total_value: Decimal) -> Result<Decimal, ContractError> {
        Ok(total_value.checked_mul(self.weight)?)
    }

    pub fn rebalance_msg(
        &self,
        address: &Addr,
        querier: &QuerierWrapper,
        total_value: Decimal,
        base_entry: &AssetAllocation<T>,
        base_balance: Uint128,
    ) -> Result<Option<CosmosMsg>, ContractError> {
        let (bal, price) = self.quote(address, querier)?;

        if price.is_zero() {
            return Ok(None);
        }

        let curr_val = Decimal::from_ratio(bal, Uint128::one()).checked_mul(price)?;
        let tgt_val = self.target_value(total_value)?;

        let mut funds: Vec<Coin> = vec![];

        if curr_val > tgt_val {
            let excess = curr_val.checked_sub(tgt_val)?;
            let sell_amt = excess.checked_div(price)?.to_uint_floor();
            if sell_amt > Uint128::zero() {
                funds = coins(sell_amt.u128(), &self.denom)
            }
        } else if curr_val < tgt_val {
            let buy_amt = tgt_val.checked_sub(curr_val)?.to_uint_floor();
            if buy_amt > Uint128::zero() && base_balance >= buy_amt {
                funds = coins(buy_amt.u128(), &base_entry.denom)
            }
        };

        let mut msg: Option<CosmosMsg> = None;

        if !funds.is_empty() {
            if let Some(swap_contract) = &self.swap_contract {
                msg = Some(
                    WasmMsg::Execute {
                        contract_addr: swap_contract.to_string(),
                        msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                            to: None,
                            min_return: None,
                            callback: None,
                        }))?,
                        funds,
                    }
                        .into(),
                );
            }
        }

        Ok(msg)
    }

    pub fn swap_msg(
        &self,
        address: &Addr,
        amount: Uint128,
        sender: &Addr,
        querier: &QuerierWrapper,
    ) -> Result<CosmosMsg, ContractError> {
        let (bal, price) = self.quote(address, querier)?;
        let sell_amt = Decimal::from_ratio(amount, Uint128::one())
            .checked_div(price)?
            .to_uint_floor();
        ensure!(
            sell_amt > Uint128::zero() && bal >= sell_amt,
            ContractError::InsufficientFunds {}
        );
        let swap_contract = self.swap_contract.clone().ok_or_else(|| {
            ContractError::Std(StdError::generic_err("Option::unwrap() called on None"))
        })?;
        Ok(WasmMsg::Execute {
            contract_addr: swap_contract.to_string(),
            msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                to: Some(sender.to_string()),
                min_return: None,
                callback: None,
            }))?,
            funds: coins(sell_amt.u128(), &self.denom),
        }
            .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractError;
    use cosmwasm_std::{
        testing::{MockApi, MockQuerier, MockStorage},
        Addr, Coin, Decimal, OwnedDeps, QuerierWrapper, Uint128, WasmMsg,
    };
    use rujira_rs::{fin::ExecuteMsg, OracleError};
    use std::str::FromStr;

    #[derive(Clone)]
    struct DummyOracle(Decimal);

    impl Oracle for DummyOracle {
        fn price(&self, _q: QuerierWrapper) -> Result<Decimal, OracleError> {
            Ok(self.0)
        }
    }

    fn setup_deps() -> OwnedDeps<MockStorage, MockApi, MockQuerier>
    {
        let balances: &[(&str, &[Coin])] = &[(
            "contract",
            &[
                Coin::new(1_000_000u128, "usdc.ETH"),
                Coin::new(10u128, "btc"),
                Coin::new(20u128, "eth"),
            ],
        )];

        let querier =
            MockQuerier::new(&[("contract", &balances[0].1)]);

        OwnedDeps {
            storage: MockStorage::new(),
            api: MockApi::default(),
            querier,
            custom_query_type: std::marker::PhantomData,
        }
    }

    #[test]
    fn test_quote() {
        let deps = setup_deps();
        let alloc = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let querier = QuerierWrapper::new(&deps.querier);
        let addr = Addr::unchecked("contract");

        let (bal, price) = alloc.quote(&addr, &querier).unwrap();
        assert_eq!(bal, Uint128::from(1_000_000u128));
        assert_eq!(price, Decimal::from_str("1.0").unwrap());
    }

    #[test]
    fn test_value() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);

        let value = allocation.value(&address, &querier).unwrap();
        assert_eq!(value, Decimal::from_ratio(1_000_000u128, 1u128));

        let allocation = AssetAllocation::new(
            "non_existing".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let value = allocation.value(&address, &querier).unwrap();
        assert_eq!(value, Decimal::zero());
    }

    #[test]
    fn test_target_value() {
        let allocation = AssetAllocation::new(
            "eth".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("eth_swap_contract")),
            DummyOracle(Decimal::from_str("2000.0").unwrap()),
        );
        let total_value = Decimal::from_ratio(2_000_000u128, 1u128);

        let target = allocation.target_value(total_value).unwrap();
        assert_eq!(target, Decimal::from_ratio(500_000u128, 1u128));

        let allocation = AssetAllocation::new(
            "eth".to_string(),
            Decimal::zero(),
            Some(Addr::unchecked("eth_swap_contract")),
            DummyOracle(Decimal::from_str("2000.0").unwrap()),
        );
        let target = allocation.target_value(total_value).unwrap();
        assert_eq!(target, Decimal::zero());
    }

    #[test]
    fn test_rebalance_msg_sell() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let base_allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let total_value = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_balance = Uint128::new(1_000_000);

        let msg = allocation
            .rebalance_msg(&address, &querier, total_value, &base_allocation, base_balance)
            .unwrap()
            .unwrap();
        if let CosmosMsg::Wasm(WasmMsg::Execute {
                                   contract_addr,
                                   msg,
                                   funds,
                               }) = msg {
            assert_eq!(contract_addr, "btc_swap_contract");
            assert_eq!(funds, vec![Coin::new(5u128, "btc")]);
            let swap_msg: ExecuteMsg = cosmwasm_std::from_json(&msg).unwrap();
            if let ExecuteMsg::Swap(swap) = swap_msg {
                assert_eq!(swap.to, None);
                assert_eq!(swap.min_return, None);
                assert_eq!(swap.callback, None);
            } else {
                panic!("Expected Swap message");
            }
        } else {
            panic!("Expected Wasm message");
        }
    }

    #[test]
    fn test_rebalance_msg_buy() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "eth".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("eth_swap_contract")),
            DummyOracle(Decimal::from_str("2000.0").unwrap()),
        );
        let base_allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let total_value = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_balance = Uint128::new(1_000_000);

        let msg = allocation
            .rebalance_msg(&address, &querier, total_value, &base_allocation, base_balance)
            .unwrap()
            .unwrap();
        if let CosmosMsg::Wasm(WasmMsg::Execute {
                                   contract_addr,
                                   msg,
                                   funds,
                               }) = msg {
            assert_eq!(contract_addr, "eth_swap_contract");
            assert_eq!(funds, vec![Coin::new(460_000u128, "usdc.ETH")]);
            let swap_msg: ExecuteMsg = cosmwasm_std::from_json(&msg).unwrap();
            if let ExecuteMsg::Swap(swap) = swap_msg {
                assert_eq!(swap.to, None);
                assert_eq!(swap.min_return, None);
                assert_eq!(swap.callback, None);
            } else {
                panic!("Expected Swap message");
            }
        } else {
            panic!("Expected Wasm message");
        }
    }

    #[test]
    fn test_rebalance_msg_no_action() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let base_allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let total_value = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_balance = Uint128::new(1_000_000);

        let msg = allocation
            .rebalance_msg(&address, &querier, total_value, &base_allocation, base_balance)
            .unwrap();
        assert_eq!(msg, None);
    }

    #[test]
    fn test_rebalance_msg_no_swap_contract() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            None,
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let base_allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let total_value = Decimal::from_ratio(2_000_000u128, 1u128);
        let base_balance = Uint128::new(1_000_000);

        let msg = allocation
            .rebalance_msg(&address, &querier, total_value, &base_allocation, base_balance)
            .unwrap();
        assert_eq!(msg, None);
    }

    #[test]
    fn test_swap_msg() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let sender = Addr::unchecked("sender");
        let querier = QuerierWrapper::new(&deps.querier);
        let amount = Uint128::new(250_000);

        let msg = allocation
            .swap_msg(&address, amount, &sender, &querier)
            .unwrap();
        if let CosmosMsg::Wasm(WasmMsg::Execute {
                                   contract_addr,
                                   msg,
                                   funds,
                               }) = msg {
            assert_eq!(contract_addr, "btc_swap_contract");
            assert_eq!(funds, vec![Coin::new(2u128, "btc")]);
            let swap_msg: ExecuteMsg = cosmwasm_std::from_json(&msg).unwrap();
            if let ExecuteMsg::Swap(swap) = swap_msg {
                assert_eq!(swap.to, Some(sender.to_string()));
                assert_eq!(swap.min_return, None);
                assert_eq!(swap.callback, None);
            } else {
                panic!("Expected Swap message");
            }
        } else {
            panic!("Expected Wasm message");
        }
    }

    #[test]
    fn test_swap_msg_insufficient_balance() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let sender = Addr::unchecked("sender");
        let querier = QuerierWrapper::new(&deps.querier);
        let amount = Uint128::new(2_000_000_000);

        let err = allocation
            .swap_msg(&address, amount, &sender, &querier)
            .unwrap_err();
        match err {
            ContractError::InsufficientFunds {} => {}
            _ => panic!("Expected ContractError::InsufficientFunds, got {:?}", err),
        }
    }

    #[test]
    fn test_swap_msg_no_swap_contract() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let sender = Addr::unchecked("sender");
        let querier = QuerierWrapper::new(&deps.querier);
        let amount = Uint128::new(250_000);

        let err = allocation
            .swap_msg(&address, amount, &sender, &querier)
            .unwrap_err();
        match err {
            ContractError::Std(StdError::GenericErr { msg, .. })
            if msg.contains("Option::unwrap()") => {}
            _ => panic!(
                "Expected ContractError::Std with 'Option::unwrap()', got {:?}", err
            ),
        }
    }

    #[test]
    fn test_value_with_large_balance() {
        let mut deps = setup_deps();
        deps.querier
            .bank
            .update_balance("contract", vec![Coin::new(u128::MAX, "btc")]);
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let result = allocation.value(&address, &querier);
        assert!(matches!(result, Err(ContractError::Overflow(_))));
    }

    #[test]
    fn test_swap_msg_exact_balance() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let sender = Addr::unchecked("sender");
        let querier = QuerierWrapper::new(&deps.querier);
        let amount = Uint128::new(1_000_000);

        let result = allocation.swap_msg(&address, amount, &sender, &querier);
        assert!(result.is_ok(), "Should succeed with exact balance");
    }

    #[test]
    fn test_rebalance_msg_zero_price() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("swap")),
            DummyOracle(Decimal::zero()),
        );
        let base_allocation = AssetAllocation::new(
            "usdc.ETH".to_string(),
            Decimal::percent(50),
            None,
            DummyOracle(Decimal::from_str("1.0").unwrap()),
        );
        let address = Addr::unchecked("contract");
        let querier = QuerierWrapper::new(&deps.querier);
        let total_value = Decimal::from_ratio(1_000u128, 1u128);
        let base_balance = Uint128::new(1_000);

        let msg = allocation
            .rebalance_msg(&address, &querier, total_value, &base_allocation, base_balance)
            .unwrap();
        assert_eq!(msg, None, "No action with zero price");
    }

    #[test]
    fn test_quote_invalid_address() {
        let deps = setup_deps();
        let allocation = AssetAllocation::new(
            "btc".to_string(),
            Decimal::percent(25),
            Some(Addr::unchecked("btc_swap_contract")),
            DummyOracle(Decimal::from_str("100000.0").unwrap()),
        );
        let address = Addr::unchecked("invalid");
        let querier = QuerierWrapper::new(&deps.querier);
        let result = allocation.quote(&address, &querier);
        assert_eq!(result.unwrap().0, Uint128::zero(), "Invalid address returns zero balance");
    }
}