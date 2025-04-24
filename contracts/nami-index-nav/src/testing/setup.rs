use cosmwasm_std::{Addr, coin, Decimal};
use cw_multi_test::{ContractWrapper, Executor};
use nami_rs::{FeeRates, OracleConfig};
use nami_rs::index_nav::InstantiateMsg;
use rujira_rs::{Chain, Layer1Asset, TokenMetadata};
use rujira_rs_testing::RujiraApp;

use crate::contract::{execute, instantiate, query, sudo};

pub struct IndexNav {
    pub address: Addr,
    pub swap_contract: Addr,
}

pub fn index_nav(app: &mut RujiraApp, base_denom: String, _target_denoms: Vec<String>) -> IndexNav {
    let owner = app.api().addr_make("owner");
    let fee_collector = app.api().addr_make("fee_collector");

    // Store swap contract code
    let swap_code = rujira_rs_testing::mock_swap_contract();
    let swap_code_id = app.store_code(swap_code);
    let swap_instance = app
        .instantiate_contract(swap_code_id, owner.clone(), &(), &[], "swap", None)
        .unwrap();

    let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
    let code_id = app.store_code(code);

    let msg = InstantiateMsg {
        base_denom: base_denom.clone(),
        fee_collector: fee_collector.to_string(),
        fees: FeeRates {
            management: Some(Decimal::percent(2)),
            performance: None,
            transaction: Some(Decimal::percent(1)),
        },
        receipt: TokenMetadata {
            name: "token".to_string(),
            symbol: "token".to_string(),
            description: "token description".to_string(),
            display: "token display".to_string(),
            uri: None,
            uri_hash: None,
        },
        target_denoms: vec![
            (
                base_denom.clone(),
                Decimal::percent(10),
                "".to_string(),
                OracleConfig::Layer1(Layer1Asset::new(
                    Chain::Eth,
                    "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                )),
            ),
            (
                "btc.btc".to_string(),
                Decimal::percent(90),
                swap_instance.to_string(),
                OracleConfig::Layer1(Layer1Asset::new(Chain::Btc, "BTC")),
            ),
        ],
    };

    let index_nav = app
        .instantiate_contract(code_id, owner.clone(), &msg, &[], "IndexNav", None)
        .unwrap();

    // Initialize swap contract with funds
    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(
                storage,
                &swap_instance,
                vec![coin(1_000_000, "eth.usdc"), coin(1_000, "btc.btc")],
            )
            .unwrap();
    });

    IndexNav {
        address: index_nav,
        swap_contract: swap_instance,
    }
}