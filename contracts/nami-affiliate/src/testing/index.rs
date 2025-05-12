use cosmwasm_std::{Coin, Decimal};
use nami_rs::affiliate::InstantiateMsg;
use nami_rs::index_nav::InstantiateMsg as IndexNavInstantiateMsg;
use nami_rs::{AssetAllocation, FeeRates, OracleConfig};
use nami_rs_testing::mock_nami_affiliate::MockNamiAffiliate;
use nami_rs_testing::mock_nami_app::NamiApp;
use nami_rs_testing::mock_nami_index_nav::MockNamiIndexNav;
use rujira_rs::{Chain, Layer1Asset, TokenMetadata};
use rujira_rs_testing::mock_rujira_app;

pub struct TestEnv {
    pub app: NamiApp,
    pub affiliate: MockNamiAffiliate,
    pub target_contract: String, // Mock nami-index-nav contract
}

pub fn setup(balances: Vec<(&str, Vec<Coin>)>) -> TestEnv {
    let mut nami_app = NamiApp::new(mock_rujira_app());

    for (addr, coins) in balances {
        nami_app.add_balance(addr, coins, true);
    }

    // Mock nami-index-nav contract as target
    let fee_collector = nami_app.api().addr_make("fee_collector").to_string();
    let target_contract = MockNamiIndexNav::new(
        &mut nami_app,
        IndexNavInstantiateMsg {
            quote_denom: "eth-usdc".to_string(),
            fee_collector,
            fees: FeeRates {
                management: None,
                performance: None,
                transaction: None,
            },
            receipt: TokenMetadata {
                name: "token".to_string(),
                symbol: "token".to_string(),
                description: "token description".to_string(),
                display: "token display".to_string(),
                uri: None,
                uri_hash: None,
            },
            target_allocation: vec![AssetAllocation::new(
                "eth-usdc".to_string(),
                Decimal::percent(100),
                None,
                OracleConfig::Layer1(Layer1Asset::new(
                    Chain::Eth,
                    "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                )),
                Decimal::zero(),
                Decimal::percent(1),
            )],
        },
    )
    .unwrap()
    .address
    .to_string();

    let affiliate = MockNamiAffiliate::new(
        &mut nami_app,
        InstantiateMsg {
            whitelist: Some(vec![target_contract.clone()]),
        },
    );

    TestEnv {
        app: nami_app,
        affiliate,
        target_contract,
    }
}
