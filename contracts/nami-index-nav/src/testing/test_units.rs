use std::str::FromStr;

use crate::testing::index;
use cosmwasm_std::{coin, coins, Decimal, Event, Uint128};
use nami_rs::{AssetAllocation, FeeManager, FeeRates, OracleConfig};
use nami_rs_testing::mock_fin::MockFin;
use rujira_rs::{
    fin::{Price, Side},
    Layer1Asset,
};

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
            (
                "btc-btc".to_string(),
                Uint128::from(21u128),
                Decimal::from_str("100100").unwrap(),
                Decimal::percent(50),
            ),
            (
                "eth-usdc".to_string(),
                Uint128::from(2_000_000u128),
                Decimal::from_str("1.001").unwrap(),
                Decimal::percent(50),
            ),
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
        coins(1_000_000, rcpt_denom.clone()),
        Some(Decimal::from_str("0.0000001").unwrap()),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("SlippageExceeded"));

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
    actual_allocations.sort_by(|a, b| a.0.cmp(&b.0));
    let mut expected_allocations = vec![
        (
            "btc-btc".to_string(),
            Uint128::from(21u128),
            Decimal::from_str("100100").unwrap(),
            Decimal::percent(33),
        ),
        (
            "eth-usdc".to_string(),
            Uint128::from(2_000_000u128),
            Decimal::from_str("1.001").unwrap(),
            Decimal::percent(34),
        ),
        (
            "eth-eth".to_string(),
            Uint128::from(100_000_000u128),
            Decimal::from_str("2500").unwrap(),
            Decimal::percent(33),
        ),
    ];
    expected_allocations.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        actual_allocations, expected_allocations,
        "All allocations should reflect updated weights"
    );

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
            (
                "btc-btc".to_string(),
                Uint128::from(20u128),
                Decimal::from_str("100100").unwrap(),
                Decimal::percent(50),
            ),
            (
                "eth-usdc".to_string(),
                Uint128::from(2_000_000u128),
                Decimal::from_str("1.001").unwrap(),
                Decimal::percent(50),
            ),
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
    let tight_slippage = Decimal::from_str("0.000001").unwrap();
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(withdraw_amount.u128(), rcpt_denom.clone()),
        Some(tight_slippage),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("SlippageExceeded"));

    // 0 slippage - should fail
    let res = test_env.index.execute_withdraw(
        &mut test_env.app,
        "user",
        coins(withdraw_amount.u128(), rcpt_denom.clone()),
        Some(Decimal::zero()),
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .root_cause()
        .to_string()
        .contains("SlippageExceeded"));

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
            .any(|(denom, _, _, _)| denom == "btc-btc"),
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
            .any(|(denom, _, _, _)| denom == "btc-btc"),
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
    actual_allocations.sort_by(|a, b| a.0.cmp(&b.0));
    let mut expected_allocations = vec![
        (
            "btc-btc".to_string(),
            Uint128::zero(), // No deposit yet, balances are zero
            Decimal::from_str("100100").unwrap(),
            Decimal::percent(30),
        ),
        (
            "eth-usdc".to_string(),
            Uint128::zero(),
            Decimal::from_str("1.001").unwrap(),
            Decimal::percent(40),
        ),
        (
            "eth-eth".to_string(),
            Uint128::zero(),
            Decimal::from_str("2500").unwrap(),
            Decimal::percent(30),
        ),
    ];
    expected_allocations.sort_by(|a, b| a.0.cmp(&b.0));
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
