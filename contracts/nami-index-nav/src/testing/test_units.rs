use std::str::FromStr;

use crate::testing::index;
use cosmwasm_std::{coin, coins, Decimal, Event, Uint128};
use nami_rs::{index_nav::AllocationResponse, AssetAllocation, FeeManager, FeeRates, OracleConfig};
use nami_rs_testing::mock_fin::MockFin;
use nami_rs_testing::mock_nami_app::NamiApp;
use nami_rs_testing::mock_nami_index_nav::MockNamiIndexNav;
use rujira_rs::{
    fin::{Price, Side},
    Layer1Asset, TokenMetadata,
};
use rujira_rs_testing::mock_rujira_app;

#[test]
fn base_lifecycle() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(20),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();
    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", test_env.index.address);

    // Successful deposit
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "5000000".to_string())]));

    // Check contract balances
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert_eq!(usdc_balance, Uint128::from(5_000_000u128));
    assert_eq!(btc_balance, Uint128::zero());

    // Check receipt balance for user
    let rcpt_balance = test_env.app.query_balance("user", &rcpt_denom, true);
    assert_eq!(rcpt_balance, Uint128::from(5_000_000u128));

    // Successful withdraw
    let withdraw_rcpt_amount = Uint128::from(1_000_000u128);
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_rcpt_amount.u128(), rcpt_denom.clone()),
            None,
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));

    // Verify user usdc balance
    let usdc_balance = test_env.app.query_balance("user", "eth-usdc", true);
    assert_eq!(
        usdc_balance,
        Uint128::from(10_000_000_000u128 - 5_000_000u128 + 1_000_000u128)
    );

    // Verify fee collector balance
    let fee_balance = test_env
        .app
        .query_balance("fee_collector", &rcpt_denom, true);
    assert_eq!(fee_balance, Uint128::zero());

    // Check status
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("1.001").unwrap());

    // Prepare for run
    let owner = test_env.app.api().addr_make("owner");
    let fair_price = Decimal::from_str("91219").unwrap();
    for (_denom, mock_fin) in test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(100_000_000_000, "eth-usdc"),
                    coin(100_000_000_000, "btc-btc"),
                ],
                fair_price,
                &[1u64, 2u64, 3u64],
                Uint128::from(1_000_000_000u128),
            )
            .unwrap();
    }

    // First run
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances1 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);

    // Second run (idempotent)
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances2 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);
    assert_eq!(
        balances1, balances2,
        "Balances should not change on idempotent run"
    );

    // Test queries
    let config = test_env.index.query_config(&mut test_env.app).unwrap();
    assert_eq!(config.quote_denom, "eth-usdc");

    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee,
        FeeManager {
            last_accrual_time: test_env.app.block_info().time,
            high_water_mark: Uint128::zero(),
            rates: FeeRates {
                management: None,
                performance: None,
                transaction: None
            }
        }
    );

    // Check status nav increase because btc oracle price is higher than swap price
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("1.026025").unwrap());
    assert_eq!(status.shares, Uint128::from(4_000_000u128));
    assert_eq!(status.total_value, Uint128::from(4_104_100u128));
    assert_eq!(
        status.allocation,
        [
            AllocationResponse {
                denom: "btc-btc".to_string(),
                swap_contract: Some(
                    "cosmwasm1mzdhwvvh22wrt07w59wxyd58822qavwkx5lcej7aqfkpqqlhaqfsgn6fq2"
                        .to_string()
                ),
                balance: Uint128::from(21u128),
                price: Decimal::from_str("100100").unwrap(),
                weight: Decimal::percent(50),
                threshold: Decimal::percent(0),
                slippage: Decimal::percent(20)
            },
            AllocationResponse {
                denom: "eth-usdc".to_string(),
                swap_contract: None,
                balance: Uint128::from(2_000_000u128),
                price: Decimal::from_str("1.001").unwrap(),
                weight: Decimal::percent(50),
                threshold: Decimal::percent(0),
                slippage: Decimal::percent(1)
            }
        ]
    );
}

#[test]
fn lifecycle() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
                coin(100_000_000_000, "eth-eth"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25), // set the slippage very high to make sure the swap succede with price 91219 and oracle price 100100
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25), // set the slippage very high to make sure the swap succede with price 91219 and oracle price 100100
            ),
        ],
        Some(Decimal::percent(1)), // 1% annual management fee
        Some(Decimal::percent(3)), // 3% transaction fee
        "quote",
    )
    .unwrap();
    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", test_env.index.address);
    let fee_collector_addr = test_env.app.api().addr_make("fee_collector").to_string();

    // Successful deposit
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "5000000".to_string())]));

    // Check contract balances
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert_eq!(usdc_balance, Uint128::from(5_000_000u128));
    assert_eq!(btc_balance, Uint128::zero());

    // Check receipt balance for user
    let rcpt_balance = test_env.app.query_balance("user", &rcpt_denom, true);
    assert_eq!(rcpt_balance, Uint128::from(5_000_000u128));

    // Rebalance to allocate 50% to btc-btc
    let owner = test_env.app.api().addr_make("owner");
    let fair_price_btc = Decimal::from_str("91219").unwrap();
    for (_denom, mock_fin) in &test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(1_000_000_000, "eth-usdc"),
                    coin(1_000_000_000, "btc-btc"),
                    coin(1_000_000_000, "eth-eth"),
                ],
                fair_price_btc,
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000u128),
            )
            .unwrap();
    }
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));

    // Check contract balances after rebalance
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert!(usdc_balance < Uint128::from(5_000_000u128));
    assert!(btc_balance > Uint128::zero());

    // Test withdrawal with tight slippage
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(5_000_000, rcpt_denom.clone()),
        Some(Decimal::from_str("0.0000001").unwrap()),
    );
    assert!(res.is_err());
    // slippage error in the fin swap
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("InsufficientReturn"));

    // Successful withdraw with transaction fee
    let withdraw_rcpt_amount = Uint128::from(1_000_000u128);
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_rcpt_amount.u128(), rcpt_denom.clone()),
            None,
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));
    res.assert_event(
        &Event::new("transfer").add_attributes(vec![("recipient", fee_collector_addr.as_str())]),
    );

    // Verify user usdc balance (adjusted for fees)
    let usdc_balance = test_env.app.query_balance("user", "eth-usdc", true);
    assert_eq!(
        usdc_balance,
        Uint128::from(10_000_000_000u128 - 5_000_000u128 + 1_008_799u128)
    );

    // Verify fee collector balance (transaction fee)
    let fee_balance = test_env
        .app
        .query_balance("fee_collector", &rcpt_denom, true);
    assert_eq!(fee_balance, Uint128::from(30_000u128)); // 3% of 1M

    // Move time for management fee (1 year)
    test_env.app.update_block(|block| {
        block.time = block.time.plus_seconds(31_536_000); // 1 year
    });

    // Trigger fee collection via deposit
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(1_000_000, "eth-usdc"))
        .unwrap();
    res.assert_event(
        &Event::new("mint").add_attributes(vec![("recipient", fee_collector_addr.as_str())]),
    );
    let fee_balance = test_env
        .app
        .query_balance("fee_collector", &rcpt_denom, true);
    assert!(
        fee_balance > Uint128::from(30_000u128),
        "Management fee should be added"
    );

    // Check status (NAV includes rebalance and fee effects)
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    // NAV ~1.03290561911120767 due to rebalance, btc price, and 1% management fee over 1 year
    assert!(
        status.nav > Decimal::from_str("1.03").unwrap()
            && status.nav < Decimal::from_str("1.04").unwrap(),
        "NAV should be in expected range after fees and rebalance"
    );

    // Second run (idempotent)
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances1 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances2 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);
    assert_eq!(
        balances1, balances2,
        "Balances should not change on idempotent run"
    );

    // Update allocations to add eth-eth and adjust weights
    let eth_eth_swap = MockFin::new(&mut test_env.app, "eth-eth", "eth-usdc");
    // Populate eth-eth swap orderbook to support rebalancing
    let fair_price_eth = Decimal::from_str("2500").unwrap();
    eth_eth_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(1_000_000_000, "eth-usdc"),
                coin(1_000_000_000, "eth-eth"),
            ],
            fair_price_eth,
            &[1u64],
            Uint128::from(1_000_000u128),
        )
        .unwrap();

    test_env
        .index
        .sudo_update_allocation(
            &mut test_env.app,
            vec![
                AssetAllocation::new(
                    "btc-btc".to_string(),
                    Decimal::percent(33),
                    Some(test_env.swaps[0].1.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(25),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(34),
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(25),
                ),
                AssetAllocation::new(
                    "eth-eth".to_string(),
                    Decimal::percent(33),
                    Some(eth_eth_swap.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("ETH", "ETH")),
                    Decimal::percent(0),
                    Decimal::percent(25), // set the slippage very high to make sure the swap succeed
                ),
            ],
        )
        .unwrap();

    // Rebalance to apply new allocation weights
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));

    // Mock balances to match original
    test_env.app.add_balance(
        test_env.index.address.as_str(),
        vec![
            coin(21, "btc-btc"),
            coin(2_000_000, "eth-usdc"),
            coin(100_000_000, "eth-eth"),
        ],
        false,
    );

    // Verify allocation
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    let mut actual_allocations = status.allocation.clone();
    actual_allocations.sort_by(|a, b| a.denom.cmp(&b.denom));
    let mut expected_allocations = vec![
        AllocationResponse {
            denom: "btc-btc".to_string(),
            swap_contract: Some(test_env.swaps[0].1.address.to_string()),
            balance: Uint128::from(21u128),
            price: Decimal::from_str("100100").unwrap(),
            weight: Decimal::percent(33),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(25),
        },
        AllocationResponse {
            denom: "eth-usdc".to_string(),
            swap_contract: None,
            balance: Uint128::from(2_000_000u128),
            price: Decimal::from_str("1.001").unwrap(),
            weight: Decimal::percent(34),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(25),
        },
        AllocationResponse {
            denom: "eth-eth".to_string(),
            swap_contract: Some(eth_eth_swap.address.to_string()),
            balance: Uint128::from(100_000_000u128),
            price: Decimal::from_str("2500").unwrap(),
            weight: Decimal::percent(33),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(25),
        },
    ];
    expected_allocations.sort_by(|a, b| a.denom.cmp(&b.denom));
    assert_eq!(actual_allocations, expected_allocations);

    // Test queries
    let config = test_env.index.query_config(&mut test_env.app).unwrap();
    assert_eq!(config.quote_denom, "eth-usdc");

    // Test fees query
    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee.rates,
        FeeRates {
            management: Some(Decimal::percent(1)),
            performance: None,
            transaction: Some(Decimal::percent(3)),
        }
    );

    // Execute sudo to update fees
    test_env
        .index
        .sudo_set_fees(
            &mut test_env.app,
            None,
            FeeRates {
                management: Some(Decimal::from_str("0.05").unwrap()),
                performance: None,
                transaction: Some(Decimal::from_str("0.06").unwrap()),
            },
        )
        .unwrap();

    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee,
        FeeManager {
            last_accrual_time: test_env.app.block_info().time,
            high_water_mark: Uint128::zero(),
            rates: FeeRates {
                management: Some(Decimal::from_str("0.05").unwrap()),
                performance: None,
                transaction: Some(Decimal::from_str("0.06").unwrap()),
            },
        }
    );
}

#[test]
fn invalid_allocation_weights() {
    let balances = vec![("user", vec![coin(10_000_000_000, "eth-usdc")])];
    let res = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(60), // Weights sum to 1.1
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "quote",
    );
    assert!(res.is_err());
    let err = res.err().unwrap();
    let err_str = err.to_string();
    assert!(err_str.contains("weight") || err_str.contains("sum"));
}

#[test]
fn cannot_remove_allocation_with_non_zero_balance() {
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        Some(Decimal::percent(1)),
        Some(Decimal::percent(3)),
        "quote",
    )
    .unwrap();

    test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();

    let owner = test_env.app.api().addr_make("owner");
    let fair_price = Decimal::from_str("91219").unwrap();
    for (_denom, mock_fin) in &test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![coin(500_000, "eth-usdc"), coin(1_000_000, "btc-btc")],
                fair_price,
                &[1u64],
                Uint128::from(100_000u128),
            )
            .unwrap();
    }
    test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();

    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert!(
        btc_balance > Uint128::zero(),
        "Contract should hold btc-btc after rebalance"
    );

    let res = test_env.index.sudo_update_allocation(
        &mut test_env.app,
        vec![AssetAllocation::new(
            "eth-usdc".to_string(),
            Decimal::percent(100),
            None,
            OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
            Decimal::percent(0),
            Decimal::percent(1),
        )],
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Weight must be zero to remove allocation"));
}

#[test]
fn add_contract_wrong_denom() {
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        Some(Decimal::percent(1)),
        Some(Decimal::percent(3)),
        "quote",
    )
    .unwrap();

    let wrong_denom_contract = MockFin::new_app_layer(&mut test_env.app, "btc-btc", "wrong_quote");

    test_env
        .index
        .sudo_update_allocation(
            &mut test_env.app,
            vec![
                AssetAllocation::new(
                    "btc-btc".to_string(),
                    Decimal::percent(50),
                    Some(wrong_denom_contract.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(50),
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(25),
                ),
            ],
        )
        .unwrap_err();
}

// The following tests are a copy of the base lifecycle tests, it checks only if the vault works with swap contracts with base denom = to vault quote denom
#[test]
fn base_lifecycle_with_base_denom() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "base",
    )
    .unwrap();
    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", test_env.index.address);

    // Successful deposit
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "5000000".to_string())]));

    // Check contract balances
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert_eq!(usdc_balance, Uint128::from(5_000_000u128));
    assert_eq!(btc_balance, Uint128::zero());

    // Check receipt balance for user
    let rcpt_balance = test_env.app.query_balance("user", &rcpt_denom, true);
    assert_eq!(rcpt_balance, Uint128::from(5_000_000u128));

    // Successful withdraw
    let withdraw_rcpt_amount = Uint128::from(1_000_000u128);
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_rcpt_amount.u128(), rcpt_denom.clone()),
            None,
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));

    // Verify user usdc balance
    let usdc_balance = test_env.app.query_balance("user", "eth-usdc", true);
    assert_eq!(
        usdc_balance,
        Uint128::from(10_000_000_000u128 - 5_000_000u128 + 1_000_000u128)
    );

    // Verify fee collector balance
    let fee_balance = test_env
        .app
        .query_balance("fee_collector", &rcpt_denom, true);
    assert_eq!(fee_balance, Uint128::zero());

    // Check status
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("1.001").unwrap());

    // Prepare for run
    let owner = test_env.app.api().addr_make("owner");

    // Add central order at fair_price
    let orders = vec![
        (
            Side::Base,
            Price::Fixed(Decimal::from_str("0.000011").unwrap()),
            Some(Uint128::from(1_000_000u128)),
        ),
        (
            Side::Quote,
            Price::Fixed(Decimal::from_str("0.000010").unwrap()),
            Some(Uint128::from(1_000_000u128)),
        ),
    ];

    test_env.swaps[0]
        .1
        .execute_order(
            &mut test_env.app,
            &owner,
            vec![
                coin(1_000_000u128, "btc-btc"),
                coin(1_000_000u128, "eth-usdc"),
            ],
            orders,
        )
        .unwrap();

    // First run
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances1 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);

    // Second run (idempotent)
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let balances2 = test_env
        .app
        .query_all_balances(&test_env.index.address.as_str(), false);
    assert_eq!(
        balances1, balances2,
        "Balances should not change on idempotent run"
    );

    // Test queries
    let config = test_env.index.query_config(&mut test_env.app).unwrap();
    assert_eq!(config.quote_denom, "eth-usdc");

    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee,
        FeeManager {
            last_accrual_time: test_env.app.block_info().time,
            high_water_mark: Uint128::zero(),
            rates: FeeRates {
                management: None,
                performance: None,
                transaction: None
            }
        }
    );

    // Check status nav increase because btc oracle price is higher than swap price
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("1.001").unwrap());
    assert_eq!(status.shares, Uint128::from(4_000_000u128));
    assert_eq!(status.total_value, Uint128::from(4_004_000u128));
    assert_eq!(
        status.allocation,
        [
            AllocationResponse {
                denom: "btc-btc".to_string(),
                swap_contract: Some(
                    "cosmwasm1mzdhwvvh22wrt07w59wxyd58822qavwkx5lcej7aqfkpqqlhaqfsgn6fq2"
                        .to_string()
                ),
                balance: Uint128::from(20u128),
                price: Decimal::from_str("100100").unwrap(),
                weight: Decimal::percent(50),
                threshold: Decimal::percent(0),
                slippage: Decimal::percent(1)
            },
            AllocationResponse {
                denom: "eth-usdc".to_string(),
                swap_contract: None,
                balance: Uint128::from(2_000_000u128),
                price: Decimal::from_str("1.001").unwrap(),
                weight: Decimal::percent(50),
                threshold: Decimal::percent(0),
                slippage: Decimal::percent(1)
            }
        ]
    );
}

#[test]
fn test_slippage_scenarios() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances.clone(),
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();
    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", test_env.index.address);

    // Deposit
    let deposit_amount = Uint128::from(5_000_000u128);
    test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(deposit_amount.u128(), "eth-usdc"),
        )
        .unwrap();

    // Populate orderbook for rebalancing
    let owner = test_env.app.api().addr_make("owner");
    let fair_price = Decimal::from_str("91219").unwrap(); // btc
    for (_denom, mock_fin) in &test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(1_000_000_000, "eth-usdc"),
                    coin(1_000_000_000, "btc-btc"),
                ],
                fair_price,
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000u128),
            )
            .unwrap();
    }

    // Rebalance to allocate 50% to btc-btc
    test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();

    // Check contract balances after rebalance
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert!(
        usdc_balance < deposit_amount,
        "USDC balance should decrease after rebalance"
    );
    assert!(
        btc_balance > Uint128::zero(),
        "BTC balance should increase after rebalance"
    );

    // 1% slippage
    let withdraw_amount = Uint128::from(1_000_000u128);
    let slippage = Decimal::percent(1);
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_amount.u128(), rcpt_denom.clone()),
            Some(slippage),
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));

    let usdc_balance_after = test_env.app.query_balance("user", "eth-usdc", true);
    let expected_min = withdraw_amount
        .checked_mul(Uint128::from(99u128))
        .unwrap()
        .checked_div(Uint128::from(100u128))
        .unwrap();
    assert!(
        usdc_balance_after >= Uint128::from(10_000_000_000u128 - 5_000_000u128) + expected_min,
        "User should receive at least min_amount with 1% slippage"
    );

    // 0.0001% slippage - should fail
    //  use a big amount to make sure the slippage is triggered when the swap is executed
    let tight_slippage = Decimal::from_str("0.000001").unwrap();
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(4_000_000u128, rcpt_denom.clone()),
        Some(tight_slippage),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("InsufficientReturn"));

    // 0 slippage - should fail
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(4_000_000u128, rcpt_denom.clone()),
        Some(Decimal::zero()),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("InsufficientReturn"));

    // 0.1% slippage
    let small_slippage = Decimal::from_str("0.001").unwrap();
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_amount.u128(), rcpt_denom.clone()),
            Some(small_slippage),
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));
    let usdc_balance_after = test_env.app.query_balance("user", "eth-usdc", true);
    let expected_min = withdraw_amount
        .checked_mul(Uint128::from(999u128))
        .unwrap()
        .checked_div(Uint128::from(1000u128))
        .unwrap();
    assert!(
        usdc_balance_after >= Uint128::from(10_000_000_000u128 - 5_000_000u128) + expected_min,
        "User should receive at least min_amount with 0.1% slippage"
    );

    // 100% slippage
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(withdraw_amount.u128(), rcpt_denom.clone()),
            Some(Decimal::one()),
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));

    // Invalid slippage (1.1) - should fail
    let invalid_slippage = Decimal::from_str("1.1").unwrap();
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(withdraw_amount.u128(), rcpt_denom.clone()),
        Some(invalid_slippage),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("Cannot Sub with given operands"));

    // 25% slippage
    let mut test_env_high_slippage = index::setup(
        balances.clone(),
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();
    test_env_high_slippage
        .index
        .execute_deposit(
            &mut test_env_high_slippage.app,
            "user",
            coins(deposit_amount.u128(), "eth-usdc"),
        )
        .unwrap();
    for (_denom, mock_fin) in &test_env_high_slippage.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env_high_slippage.app,
                &owner,
                vec![
                    coin(1_000_000_000, "eth-usdc"),
                    coin(1_000_000_000, "btc-btc"),
                ],
                fair_price, // 91219
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000u128),
            )
            .unwrap();
    }
    let res = test_env_high_slippage
        .index
        .execute_run(&mut test_env_high_slippage.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));
    let btc_balance = test_env_high_slippage.app.query_balance(
        &test_env_high_slippage.index.address.as_str(),
        "btc-btc",
        false,
    );
    assert!(
        btc_balance > Uint128::zero(),
        "Rebalance should swap to BTC with high slippage"
    );
}

#[test]
fn test_add_allocation_denom_validation() {
    // Initialize user balances
    let balances = vec![
        (
            "owner",
            vec![
                coin(300_000_000_000, "eth-usdc"),
                coin(200_000_000_000, "btc-btc"),
                coin(100_000_000_000, "wrong-denom"),
                coin(100_000_000_000, "invalid-denom"),
            ],
        ),
        (
            "user",
            vec![
                coin(10_000_000_000, "eth-usdc"),
                coin(10_000_000_000, "btc-btc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();
    let owner = test_env.app.api().addr_make("owner");

    // Populate existing swap mocks
    test_env.swaps.iter().for_each(|(denom, swap_mock)| {
        let quote_denom = "eth-usdc".to_string();
        let base_denom = denom.clone();
        swap_mock
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(100_000_000_000, quote_denom.clone()),
                    coin(100_000_000_000, base_denom),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(1_000_000_000u128),
            )
            .unwrap();
    });

    // Valid pair (quote_denom = eth-usdc, denom = btc-btc)
    let btc_btc_swap = test_env
        .swaps
        .iter()
        .find(|(denom, _)| denom == "btc-btc")
        .unwrap();
    test_env
        .index
        .sudo_update_allocation(
            &mut test_env.app,
            vec![
                AssetAllocation::new(
                    "btc-btc".to_string(),
                    Decimal::percent(50),
                    Some(btc_btc_swap.1.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(50),
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
            ],
        )
        .unwrap();
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert!(
        status
            .allocation
            .iter()
            .any(|allocation| allocation.denom == "btc-btc"),
        "BTC-btc allocation not found"
    );

    // Valid flipped pair (quote_denom = eth-usdc, denom = btc-btc, swap: quote = btc-btc, base = eth-usdc)
    let flipped_swap =
        nami_rs_testing::mock_fin::MockFin::new_app_layer(&mut test_env.app, "eth-usdc", "btc-btc");
    flipped_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(100_000_000_000, "btc-btc"),
                coin(100_000_000_000, "eth-usdc"),
            ],
            Decimal::from_str("100").unwrap(),
            &[1u64, 2u64, 3u64],
            Uint128::from(1_000_000_000u128),
        )
        .unwrap();
    test_env
        .index
        .sudo_update_allocation(
            &mut test_env.app,
            vec![
                AssetAllocation::new(
                    "btc-btc".to_string(),
                    Decimal::percent(50),
                    Some(flipped_swap.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(50),
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
            ],
        )
        .unwrap();
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert!(
        status
            .allocation
            .iter()
            .any(|allocation| allocation.denom == "btc-btc"),
        "BTC-btc allocation (flipped) not found"
    );

    // Invalid pair (swap with wrong quote denom)
    let invalid_swap = nami_rs_testing::mock_fin::MockFin::new_app_layer(
        &mut test_env.app,
        "invalid-denom",
        "wrong-denom",
    );
    invalid_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(100_000_000_000, "wrong-denom"),
                coin(100_000_000_000, "invalid-denom"),
            ],
            Decimal::from_str("100").unwrap(),
            &[1u64, 2u64, 3u64],
            Uint128::from(1_000_000_000u128),
        )
        .unwrap();
    let res = test_env
        .index
        .sudo_update_allocation(
            &mut test_env.app,
            vec![
                AssetAllocation::new(
                    "invalid-denom".to_string(),
                    Decimal::percent(50),
                    Some(invalid_swap.address.to_string()),
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(50),
                    None,
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
            ],
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Invalid denom pair");
}

#[test]
fn test_update_allocations() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
                coin(100_000_000_000, "eth-eth"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(1),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();

    // Successful allocation update
    println!("I'm here");
    let eth_eth_swap = MockFin::new(&mut test_env.app, "eth-eth", "eth-usdc");
    println!("I'm not here");
    let fair_price_eth = Decimal::from_str("2500").unwrap();
    let owner = test_env.app.api().addr_make("owner");
    eth_eth_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(1_000_000_000, "eth-usdc"),
                coin(1_000_000_000, "eth-eth"),
            ],
            fair_price_eth,
            &[1u64],
            Uint128::from(1_000_000u128),
        )
        .unwrap();

    let new_allocations = vec![
        AssetAllocation::new(
            "btc-btc".to_string(),
            Decimal::percent(30),
            Some(test_env.swaps[0].1.address.to_string()),
            OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
        AssetAllocation::new(
            "eth-usdc".to_string(),
            Decimal::percent(40),
            None,
            OracleConfig::Layer1(Layer1Asset::new(
                "ETH",
                "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
            )),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
        AssetAllocation::new(
            "eth-eth".to_string(),
            Decimal::percent(30),
            Some(eth_eth_swap.address.to_string()),
            OracleConfig::Layer1(Layer1Asset::new("ETH", "ETH")),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
    ];

    test_env
        .index
        .sudo_update_allocation(&mut test_env.app, new_allocations.clone())
        .unwrap();

    // Verify allocation weights
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    let mut actual_allocations = status.allocation.clone();
    actual_allocations.sort_by(|a, b| a.denom.cmp(&b.denom));
    let mut expected_allocations = vec![
        AllocationResponse {
            denom: "btc-btc".to_string(),
            swap_contract: Some(test_env.swaps[0].1.address.to_string()),
            balance: Uint128::zero(), // No deposit yet, balances are zero
            price: Decimal::from_str("100100").unwrap(),
            weight: Decimal::percent(30),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(1),
        },
        AllocationResponse {
            denom: "eth-usdc".to_string(),
            swap_contract: None,
            balance: Uint128::zero(),
            price: Decimal::from_str("1.001").unwrap(),
            weight: Decimal::percent(40),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(1),
        },
        AllocationResponse {
            denom: "eth-eth".to_string(),
            swap_contract: Some(eth_eth_swap.address.to_string()),
            balance: Uint128::zero(),
            price: Decimal::from_str("2500").unwrap(),
            weight: Decimal::percent(30),
            threshold: Decimal::percent(0),
            slippage: Decimal::percent(1),
        },
    ];
    expected_allocations.sort_by(|a, b| a.denom.cmp(&b.denom));
    assert_eq!(
        actual_allocations, expected_allocations,
        "Allocations should reflect updated weights"
    );

    // Failure when weights do not sum to 1.0
    let invalid_allocations = vec![
        AssetAllocation::new(
            "btc-btc".to_string(),
            Decimal::percent(50),
            Some(test_env.swaps[0].1.address.to_string()),
            OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
        AssetAllocation::new(
            "eth-usdc".to_string(),
            Decimal::percent(60),
            None,
            OracleConfig::Layer1(Layer1Asset::new(
                "ETH",
                "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
            )),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
    ];
    let res = test_env
        .index
        .sudo_update_allocation(&mut test_env.app, invalid_allocations);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Weight must sum to 1"));

    // Failure when removing allocation with non-zero balance
    test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();
    let fair_price_btc = Decimal::from_str("91219").unwrap();
    let owner = test_env.app.api().addr_make("owner");
    for (_denom, mock_fin) in &test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(1_000_000_000, "eth-usdc"),
                    coin(1_000_000_000, "btc-btc"),
                ],
                fair_price_btc,
                &[1u64],
                Uint128::from(1_000_000u128),
            )
            .unwrap();
    }
    test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();

    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert!(
        btc_balance > Uint128::zero(),
        "Contract should hold btc-btc after rebalance"
    );

    let remove_btc_allocation = vec![AssetAllocation::new(
        "eth-usdc".to_string(),
        Decimal::percent(100),
        None,
        OracleConfig::Layer1(Layer1Asset::new(
            "ETH",
            "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
        )),
        Decimal::percent(0),
        Decimal::percent(1),
    )];
    let res = test_env
        .index
        .sudo_update_allocation(&mut test_env.app, remove_btc_allocation);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Weight must be zero to remove allocation"));

    // Failure with multiple quote denominations
    let invalid_quote_allocations = vec![
        AssetAllocation::new(
            "btc-btc".to_string(),
            Decimal::percent(50),
            None,
            OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
        AssetAllocation::new(
            "eth-usdc".to_string(),
            Decimal::percent(50),
            None,
            OracleConfig::Layer1(Layer1Asset::new(
                "ETH",
                "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
            )),
            Decimal::percent(0),
            Decimal::percent(1),
        ),
    ];
    let res = test_env
        .index
        .sudo_update_allocation(&mut test_env.app, invalid_quote_allocations);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Missing or duplicate quote allocation"));
}

#[test]
fn test_minting_receipt() {
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "btc-btc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
            ],
        ),
    ];

    let mut nami_app = NamiApp::new(mock_rujira_app());

    for (addr, coins) in balances {
        nami_app.add_balance(addr, coins, true);
    }

    let fee_collector = nami_app.api().addr_make("fee_collector");
    let swap_mock = MockFin::new(&mut nami_app, "btc-btc", "eth-usdc");

    let index = MockNamiIndexNav::new(
        &mut nami_app,
        nami_rs::index_nav::InstantiateMsg {
            quote_denom: "btc-btc".to_string(),
            fee_collector: fee_collector.to_string(),
            fees: FeeRates {
                management: None,
                performance: None,
                transaction: None,
            },
            receipt: TokenMetadata {
                name: "".to_string(),
                symbol: "".to_string(),
                description: "".to_string(),
                display: "".to_string(),
                uri: None,
                uri_hash: None,
            },
            target_allocation: vec![
                AssetAllocation::new(
                    "btc-btc".to_string(),
                    Decimal::percent(50),
                    None,
                    // price of oracle is 100100
                    OracleConfig::Layer1(Layer1Asset::new("BTC", "BTC")),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
                AssetAllocation::new(
                    "eth-usdc".to_string(),
                    Decimal::percent(50),
                    Some(swap_mock.address.to_string()),
                    // price of oracle is 1.001
                    OracleConfig::Layer1(Layer1Asset::new(
                        "ETH",
                        "USDC-0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
                    )),
                    Decimal::percent(0),
                    Decimal::percent(1),
                ),
            ],
        },
    )
    .unwrap();

    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", index.address);

    // Check NAV before deposit should always be the price of the quote denom if no rebalance or no time passed
    let status = index.query_status(&mut nami_app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("100100").unwrap());

    index
        .execute_deposit(&mut nami_app, "user", coins(50u128, "btc-btc"))
        .unwrap();

    // Check NAV after deposit should always be the price of the quote denom if no rebalance or no time passed
    let status = index.query_status(&mut nami_app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("100100").unwrap());

    // total shares should be 50
    assert_eq!(status.shares, Uint128::from(50u128));

    // check the balance of user on shares
    let balance = nami_app.query_balance("user", &rcpt_denom, true);
    assert_eq!(balance, Uint128::from(50u128));

    index
        .execute_deposit(&mut nami_app, "user", coins(50u128, "btc-btc"))
        .unwrap();

    // check the balance of user on shares
    let balance = nami_app.query_balance("user", &rcpt_denom, true);
    assert_eq!(balance, Uint128::from(100u128));

    // Check NAV after deposit should stay always be the price of the quote denom if no rebalance or no time passed
    let status = index.query_status(&mut nami_app).unwrap();
    assert_eq!(status.nav, Decimal::from_str("100100").unwrap());
    // total value should be 100 * 100100 = 10010000
    assert_eq!(status.total_value, Uint128::from(10_010_000u128));

    // total shares should be 100
    assert_eq!(status.shares, Uint128::from(100u128));

    let owner = nami_app.api().addr_make("owner");
    let order = vec![(
        Side::Quote,
        Price::Fixed(Decimal::from_str("100100").unwrap()),
        Some(Uint128::from(100_000_000_000u128)),
    )];
    swap_mock
        .execute_order(
            &mut nami_app,
            &owner,
            vec![coin(100_000_000_000, "eth-usdc")],
            order,
        )
        .unwrap();

    // Run should swap 100 / 2 = 50 btc in eth-usdc receiving 50 * 100100 = 5_005_000 eth-usdc
    index.execute_run(&mut nami_app, "user").unwrap();

    let usdc_balance = nami_app.query_balance(&index.address.as_str(), "eth-usdc", false);
    assert_eq!(usdc_balance, Uint128::from(5_005_000u128));

    // Check NAV after run should not reduce if no fee are taken from the orderbook
    // Total value should be
    // 5_005_000 usdc * 1.001 = 5_010_005 usd
    // 50 btc * 100_100 = 5_005_000 usd
    // Total value = 5_010_005 + 5_005_000 = 10_015_005 usd
    let status = index.query_status(&mut nami_app).unwrap();
    assert_eq!(status.total_value, Uint128::from(10_015_005u128));
    assert_eq!(status.shares, Uint128::from(100u128));

    // NAV should be 10_015_005 / 100 = 100_150.05
    assert_eq!(status.nav, Decimal::from_str("100150.05").unwrap());

    // New deposit
    index
        .execute_deposit(&mut nami_app, "user", coins(50u128, "btc-btc"))
        .unwrap();

    // Check NAV after deposit should stay always be the price of the quote denom if no rebalance or no time passed
    let status = index.query_status(&mut nami_app).unwrap();

    // total shares should be
    // old shares = 100
    // new shares =  amount * price / nav
    // new shares = 50 * 100100 / 100150.05 = 49.95 = 49 rounded floor
    // total shares = 100 + 49 = 149
    assert_eq!(status.shares, Uint128::from(149u128));

    // check the balance of user on shares
    let balance = nami_app.query_balance("user", &rcpt_denom, true);
    assert_eq!(balance, Uint128::from(149u128));

    // total value should be previous total value + new deposit value
    // previous total value = 10_015_005
    // new deposit value = 50 * 100100 = 5_005_000
    // total value = 10_015_005 + 5_005_000 = 15_020_005

    // NAV should be 15_020_005 / 149 = 100_805.402684563758389261
    // increase of nav due to the rounding floor
    assert_eq!(
        status.nav,
        Decimal::from_str("100805.402684563758389261").unwrap()
    );

    // total value from the status query is calculated as nav * shares
    let total = Decimal::from_ratio(149u128, 1u128)
        .checked_mul(Decimal::from_str("100805.402684563758389261").unwrap())
        .unwrap()
        .to_uint_floor();
    assert_eq!(total, status.total_value);
}

#[test]
fn edge_case_withdraw_all() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
        (
            "owner",
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "btc-btc"),
                coin(100_000_000_000, "eth-eth"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            (
                "btc-btc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25),
            ),
            (
                "eth-usdc".to_string(),
                Decimal::percent(50),
                Decimal::percent(0),
                Decimal::percent(25),
            ),
        ],
        None,
        None,
        "quote",
    )
    .unwrap();
    let rcpt_denom = format!("x/nami-index-nav-{}-rcpt", test_env.index.address);
    let fee_collector_addr = test_env.app.api().addr_make("fee_collector").to_string();

    // Successful deposit
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", coins(5_000_000u128, "eth-usdc"))
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "5000000".to_string())]));

    // Check contract balances
    let usdc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    let btc_balance =
        test_env
            .app
            .query_balance(&test_env.index.address.as_str(), "btc-btc", false);
    assert_eq!(usdc_balance, Uint128::from(5_000_000u128));
    assert_eq!(btc_balance, Uint128::zero());

    // Check receipt balance for user
    let rcpt_balance = test_env.app.query_balance("user", &rcpt_denom, true);
    assert_eq!(rcpt_balance, Uint128::from(5_000_000u128));

    // Rebalance to allocate 50% to btc-btc
    let owner = test_env.app.api().addr_make("owner");
    let fair_price_btc = Decimal::from_str("100100").unwrap();
    for (_denom, mock_fin) in &test_env.swaps {
        mock_fin
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(1_000_000_000, "eth-usdc"),
                    coin(1_000_000_000, "btc-btc"),
                    coin(1_000_000_000, "eth-eth"),
                ],
                fair_price_btc,
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000u128),
            )
            .unwrap();
    }
    let res = test_env
        .index
        .execute_run(&mut test_env.app, "user")
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-nav/run"));

    // Test withdrawal all
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(5_000_000, rcpt_denom.clone()),
            None,
        )
        .unwrap();

    res.assert_event(&Event::new("wasm-nami-index-nav/withdraw"));
    res.assert_event(&Event::new("burn"));

    // Check status (NAV should go back to 1,001, share should be 0)
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    println!("status: {:#?}", status);
    assert_eq!(status.nav, Decimal::from_str("1.001").unwrap());
    assert_eq!(status.shares, Uint128::zero());
}
