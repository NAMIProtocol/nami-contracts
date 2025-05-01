use cosmwasm_std::{Coin, Decimal};
use nami_rs::index_nav::InstantiateMsg;
use nami_rs::{AssetAllocation, FeeRates, OracleConfig};
use nami_rs_testing::mock_fin::MockFin;
use nami_rs_testing::mock_nami_app::NamiApp;
use nami_rs_testing::mock_nami_index_nav::MockNamiIndexNav;
use rujira_rs::{Chain, Layer1Asset, TokenMetadata};
use rujira_rs_testing::mock_rujira_app;

pub struct TestEnv {
    pub app: NamiApp,
    pub index: MockNamiIndexNav,
    pub swaps: Vec<(String, MockFin)>,
}

pub fn setup(
    balances: Vec<(&str, Vec<Coin>)>,
    quote_denom: String,
    target_allocations: Vec<(String, Decimal, Decimal)>,
    management_fee: Option<Decimal>,
    transaction_fee: Option<Decimal>,
) -> anyhow::Result<TestEnv> {
    let mut nami_app = NamiApp::new(mock_rujira_app());

    for (addr, coins) in balances {
        nami_app.add_balance(addr, coins, true);
    }
    let fee_collector = nami_app.api().addr_make("fee_collector");

    let (swap_mocks, target_allocations) =
        get_target_allocations(&mut nami_app, target_allocations);

    let index = MockNamiIndexNav::new(
        &mut nami_app,
        InstantiateMsg {
            quote_denom: quote_denom.clone(),
            fee_collector: fee_collector.to_string(),
            fees: FeeRates {
                management: management_fee,
                performance: None,
                transaction: transaction_fee,
            },
            receipt: TokenMetadata {
                name: "".to_string(),
                symbol: "".to_string(),
                description: "".to_string(),
                display: "".to_string(),
                uri: None,
                uri_hash: None,
            },
            target_allocation: target_allocations,
        },
    )?;

    Ok(TestEnv {
        app: nami_app,
        index,
        swaps: swap_mocks,
    })
}

fn get_target_allocations(
    app: &mut NamiApp,
    target_allocations: Vec<(String, Decimal, Decimal)>,
) -> (Vec<(String, MockFin)>, Vec<AssetAllocation<OracleConfig>>) {
    let mut result = Vec::new();
    let mut swap_mocks: Vec<(String, MockFin)> = Vec::new();
    for (denom, weight, threshold) in target_allocations {
        match denom.as_str() {
            "btc-btc" => {
                let swap_mock = MockFin::new(app, "btc-btc");
                result.push(AssetAllocation::new(
                    denom.clone(),
                    weight,
                    Some(swap_mock.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new(Chain::Btc, "BTC")),
                    threshold,
                ));
                swap_mocks.push((denom.clone(), swap_mock));
            }
            "eth-eth" => {
                let swap_mock = MockFin::new(app, "eth-eth");
                result.push(AssetAllocation::new(
                    denom.clone(),
                    weight,
                    Some(swap_mock.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new(Chain::Eth, "ETH")),
                    threshold,
                ));
                swap_mocks.push((denom.clone(), swap_mock));
            }
            "eth-usdc" => {
                result.push(AssetAllocation::new(
                    denom.clone(),
                    weight,
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        Chain::Eth,
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    threshold,
                ));
            }
            _ => {}
        }
    }
    (swap_mocks, result)
}
