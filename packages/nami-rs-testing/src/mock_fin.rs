use std::str::FromStr;

use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};
use rujira_rs::{
    fin::{Denoms, ExecuteMsg, InstantiateMsg, Price, Side, Tick},
    Chain, Layer1Asset,
};
use rujira_rs_testing::RujiraApp;

use rujira_fin::contract::{execute, instantiate, query, sudo};

pub struct MockFin {
    pub address: Addr,
}

impl MockFin {
    pub fn new(app: &mut RujiraApp, base_denom: &str) -> Self {
        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let layer_1_asset = get_layer_1_asset(base_denom);
        let contract = app
            .instantiate_contract(
                code_id,
                owner,
                &InstantiateMsg {
                    denoms: Denoms::new(base_denom, "eth-usdc"),
                    market_maker: None,
                    oracles: Some([
                        layer_1_asset,
                        Layer1Asset::try_from(
                            "ETH.USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                        )
                        .unwrap(),
                    ]),
                    tick: Tick::new(6u8),
                    fee_taker: Decimal::zero(),
                    fee_maker: Decimal::zero(),
                    fee_address: app.api().addr_make("fee").to_string(),
                },
                &[],
                "template",
                None,
            )
            .unwrap();

        MockFin { address: contract }
    }

    pub fn new_app_layer(app: &mut RujiraApp, base_denom: &str) -> Self {
        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let contract = app
            .instantiate_contract(
                code_id,
                owner,
                &InstantiateMsg {
                    denoms: Denoms::new(base_denom, "eth-usdc"),
                    market_maker: None,
                    oracles: None,
                    tick: Tick::new(6u8),
                    fee_taker: Decimal::zero(),
                    fee_maker: Decimal::zero(),
                    fee_address: app.api().addr_make("fee").to_string(),
                },
                &[],
                "template",
                None,
            )
            .unwrap();

        MockFin { address: contract }
    }

    pub fn new_app_layer_wrong_quote_denom(app: &mut RujiraApp, base_denom: &str) -> Self {
        let owner = app.api().addr_make("owner");

        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let contract = app
            .instantiate_contract(
                code_id,
                owner,
                &InstantiateMsg {
                    denoms: Denoms::new(base_denom, "wrong-denom"),
                    market_maker: None,
                    oracles: None,
                    tick: Tick::new(6u8),
                    fee_taker: Decimal::zero(),
                    fee_maker: Decimal::zero(),
                    fee_address: app.api().addr_make("fee").to_string(),
                },
                &[],
                "template",
                None,
            )
            .unwrap();

        MockFin { address: contract }
    }

    pub fn populate_orderbooks(
        &self,
        app: &mut RujiraApp,
        user: &Addr,
        funds: Vec<Coin>,
    ) -> anyhow::Result<AppResponse> {
        app.execute_contract(
            user.clone(),
            self.address.clone(),
            &ExecuteMsg::Order((
                vec![
                    (Side::Base, Price::Oracle(0), Some(Uint128::from(10000u128))),
                    (
                        Side::Base,
                        Price::Fixed(Decimal::from_str("100000").unwrap()),
                        Some(Uint128::from(10000u128)),
                    ),
                    (
                        Side::Base,
                        Price::Fixed(Decimal::from_str("93317").unwrap()),
                        Some(Uint128::from(10000u128)),
                    ),
                    (
                        Side::Base,
                        Price::Fixed(Decimal::from_str("93219").unwrap()),
                        Some(Uint128::from(21000u128)),
                    ),
                    (
                        Side::Base,
                        Price::Fixed(Decimal::from_str("91219").unwrap()),
                        Some(Uint128::from(51000u128)),
                    ),
                    (
                        Side::Quote,
                        Price::Oracle(-1000),
                        Some(Uint128::from(1000000000u128)),
                    ),
                    (
                        Side::Quote,
                        Price::Fixed(Decimal::from_str("90000").unwrap()),
                        Some(Uint128::from(1250000000u128)),
                    ),
                    (
                        Side::Quote,
                        Price::Fixed(Decimal::from_str("87900").unwrap()),
                        Some(Uint128::from(5100000000u128)),
                    ),
                ],
                None,
            )),
            &funds,
        )
    }

    pub fn populate_orderbook(
        &self,
        app: &mut RujiraApp,
        user: &Addr,
        funds: Vec<Coin>,
        fair_price: Decimal,
        levels: &[u64],
        size: Uint128,
    ) -> anyhow::Result<AppResponse> {
        // Generate price levels based on percent offsets
        let mut orders = Vec::with_capacity(levels.len() * 2 + 1);

        // Add central order at fair_price
        orders.push((Side::Base, Price::Fixed(fair_price), Some(size)));

        // Add symmetric orders above and below fair_price
        for &pct in levels {
            let offset = fair_price * Decimal::percent(pct as u64);
            let price_above = (fair_price + offset).floor();
            let price_below = (fair_price - offset).ceil();
            orders.push((Side::Base, Price::Fixed(price_above), Some(size)));
            orders.push((Side::Quote, Price::Fixed(price_below), Some(size)));
        }

        app.execute_contract(
            user.clone(),
            self.address.clone(),
            &ExecuteMsg::Order((orders, None)),
            &funds,
        )
    }
}

fn get_layer_1_asset(denom: &str) -> Layer1Asset {
    match denom {
        "btc-btc" => Layer1Asset::new(Chain::Btc, "BTC"),
        "eth-usdc" => {
            Layer1Asset::try_from("ETH.USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48").unwrap()
        }
        "eth-eth" => Layer1Asset::new(Chain::Eth, "ETH"),
        _ => panic!("Invalid denom"),
    }
}
