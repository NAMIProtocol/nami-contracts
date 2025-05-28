use cosmwasm_std::{Coin, Decimal, Uint128};
use nami_rs::affiliate;
use nami_rs::index_entry_adapter::InstantiateMsg;
use nami_rs::index_fixed;
use nami_rs::FeeRates;
use nami_rs_testing::mock_fin::MockFin;
use nami_rs_testing::mock_nami_affiliate::MockNamiAffiliate;
use nami_rs_testing::mock_nami_app::NamiApp;
use nami_rs_testing::mock_nami_index_entry_adapter::MockNamiIndexEntryAdapter;
use nami_rs_testing::mock_nami_index_fixed::MockNamiIndexFixed;
use rujira_rs::TokenMetadata;
use rujira_rs_testing::mock_rujira_app;

pub struct TestEnv {
    pub app: NamiApp,
    pub affiliate: MockNamiAffiliate,
    pub entry_adapter: MockNamiIndexEntryAdapter,
    pub index: MockNamiIndexFixed,
    pub swaps: Vec<(String, MockFin)>,
}

pub fn setup(
    balances: Vec<(&str, Vec<Coin>)>,
    quote_denom: String,
    target_allocations: Vec<(String, Uint128)>,
    management_fee: Option<Decimal>,
    transaction_fee: Option<Decimal>,
) -> TestEnv {
    let mut nami_app = NamiApp::new(mock_rujira_app());

    for (addr, coins) in balances {
        nami_app.add_balance(addr, coins, true);
    }
    let fee_collector = nami_app.api().addr_make("fee_collector");

    let (swap_mocks, target_allocations) =
        get_target_allocations(&mut nami_app, target_allocations);

    let index = MockNamiIndexFixed::new(
        &mut nami_app,
        index_fixed::InstantiateMsg {
            receipt: TokenMetadata {
                name: "".to_string(),
                symbol: "".to_string(),
                description: "".to_string(),
                display: "".to_string(),
                uri: None,
                uri_hash: None,
            },
            quote_denom: quote_denom.clone(),
            fee_collector: fee_collector.to_string(),
            fees: FeeRates {
                management: management_fee,
                performance: None,
                transaction: transaction_fee,
            },
            target_allocations,
        },
    );

    let entry_adapter = MockNamiIndexEntryAdapter::new(
        &mut nami_app,
        InstantiateMsg {
            quote_denom: quote_denom.clone(),
            swap_contracts: swap_mocks
                .iter()
                .map(|(denom, swap_mock)| (denom.clone(), swap_mock.address.to_string()))
                .collect(),
        },
    );

    let affiliate = MockNamiAffiliate::new(
        &mut nami_app,
        affiliate::InstantiateMsg {
            max_affiliate_fee_bps: 2000,
            whitelist: Some(vec![entry_adapter.address.to_string()]),
        },
    );

    TestEnv {
        app: nami_app,
        affiliate,
        entry_adapter,
        index,
        swaps: swap_mocks,
    }
}

fn get_target_allocations(
    app: &mut NamiApp,
    target_allocations: Vec<(String, Uint128)>,
) -> (Vec<(String, MockFin)>, Vec<(String, Uint128, String)>) {
    let mut result = Vec::new();
    let mut swap_mocks: Vec<(String, MockFin)> = Vec::new();
    for (denom, weight) in target_allocations {
        let swap_mock = MockFin::new_app_layer(app, denom.as_str(), "eth-usdc");
        result.push((denom.clone(), weight, swap_mock.address.to_string()));
        swap_mocks.push((denom.clone(), swap_mock));
    }
    (swap_mocks, result)
}
