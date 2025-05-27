use std::str::FromStr;

use crate::testing::index;
use cosmwasm_std::{coin, Decimal, Event, Uint128};
use cosmwasm_std::{to_json_binary, Binary, Empty};
use cw_multi_test::Executor;
use nami_rs::index_fixed::{CallbackType, ExecuteMsg};
use nami_rs::FeeManager;
use nami_rs::FeeRates;
use nami_rs_testing::mock_fin::MockFin;
use rujira_rs::CallbackData;
use rujira_rs::CallbackMsg;

#[test]
fn base_test() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(50u128)),
            ("auto".to_string(), Uint128::from(100u128)),
            ("lqdy".to_string(), Uint128::from(20u128)),
        ],
        None,
        None,
    );
    let rcpt_denom = format!("x/nami-index-fixed-{}-rcpt", test_env.index.address);

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(50u128, "nami"),
                coin(100u128, "auto"),
                coin(20u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "1".to_string())]));

    // Successful withdraw correct proportion
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            vec![coin(1u128, rcpt_denom.clone())],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/withdraw"));
    res.assert_event(&Event::new("burn").add_attributes(vec![("amount", "1".to_string())]));
}

#[test]
fn lifecycle() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000_000u128)),
            ("auto".to_string(), Uint128::from(100_000_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );
    let rcpt_denom = format!("x/nami-index-fixed-{}-rcpt", test_env.index.address);

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000_000u128, "nami"),
                coin(10_000_000_000u128, "auto"),
                coin(10_000_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // query status to see the inflation Zero because no execution
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));

    // query balances we should have 100 shares * 100 = 10000 nami - 100 shares * 100 = 10000 auto - 100 shares * 100 = 10000 lqdy
    let res = test_env
        .app
        .query_all_balances(test_env.index.address.as_str(), false);
    assert_eq!(res[0].amount, Uint128::from(10_000_000_000u128));
    assert_eq!(res[1].amount, Uint128::from(10_000_000_000u128));
    assert_eq!(res[2].amount, Uint128::from(10_000_000_000u128));

    // move block 1 year
    const SECS_PER_YEAR: u64 = 31_557_600; // ≈365.25 days
    test_env.app.update_block(|block| {
        block.height += 1;
        block.time = block.time.plus_seconds(SECS_PER_YEAR);
    });

    //  execute any action (withdraw) to mint the inflation
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            vec![coin(100u128, rcpt_denom.clone())],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/withdraw"));

    // query status to see the inflation + withdraw fee 2% = 200_000. Allocation is now reduced due to the inflation
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(1u128));
    assert_eq!(
        res.allocation,
        vec![
            ("auto".to_string(), Uint128::from(99009900u128)),
            ("lqdy".to_string(), Uint128::from(99009900u128)),
            ("nami".to_string(), Uint128::from(99009900u128)),
        ]
    );

    // query balances we should have 1 shares * 99 = 99 auto - 1 shares * 99 = 99 lqdy - 1 shares * 99 = 99 nami
    let res = test_env
        .app
        .query_all_balances(test_env.index.address.as_str(), false);
    assert_eq!(res[0].amount, Uint128::from(99010000u128));
    assert_eq!(res[1].amount, Uint128::from(99010000u128));
    assert_eq!(res[2].amount, Uint128::from(99010000u128));
}

#[test]
fn lifecycle_deposit_first() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000_000u128)),
            ("auto".to_string(), Uint128::from(100_000_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000_000u128, "nami"),
                coin(10_000_000_000u128, "auto"),
                coin(10_000_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // query status to see the inflation Zero because no execution
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));

    let res = test_env
        .app
        .query_all_balances(test_env.index.address.as_str(), false);
    assert_eq!(res[0].amount, Uint128::from(10_000_000_000u128));
    assert_eq!(res[1].amount, Uint128::from(10_000_000_000u128));
    assert_eq!(res[2].amount, Uint128::from(10_000_000_000u128));

    // move block 1 year
    const SECS_PER_YEAR: u64 = 31_557_600; // ≈365.25 days
    test_env.app.update_block(|block| {
        block.height += 1;
        block.time = block.time.plus_seconds(SECS_PER_YEAR);
    });

    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000_000u128, "nami"),
                coin(10_000_000_000u128, "auto"),
                coin(10_000_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(201u128));
    assert_eq!(
        res.allocation,
        vec![
            ("auto".to_string(), Uint128::from(99502487u128)),
            ("lqdy".to_string(), Uint128::from(99502487u128)),
            ("nami".to_string(), Uint128::from(99502487u128)),
        ]
    );

    let res = test_env
        .app
        .query_all_balances(test_env.index.address.as_str(), false);
    assert_eq!(res[0].amount, Uint128::from(20_000_000_000u128));
    assert_eq!(res[1].amount, Uint128::from(20_000_000_000u128));
    assert_eq!(res[2].amount, Uint128::from(20_000_000_000u128));
}

#[test]
fn test_reallocate() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(500_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000u128, "nami"),
                coin(10_000_000u128, "auto"),
                coin(10_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // query status to see the inflation Zero because no execution
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));

    // populate orderbooks
    let owner = test_env.app.api().addr_make("owner");
    test_env.swaps.iter().for_each(|swap| {
        swap.1
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(100_000_000_000, "eth-usdc"),
                    coin(100_000_000_000, swap.0.clone()),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000_000u128),
            )
            .unwrap();
    });

    //  try reallocation
    let res = test_env
        .index
        .sudo_reallocate(
            &mut test_env.app,
            "auto",
            "lqdy",
            Uint128::from(50_000u128),
            None,
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/reallocate"));

    // query status to see the new allocation
    // selling auto for usdc price 0.99 gets 495_000 usdc
    // selling usdc for lqdy price 1.00 gets 495_000 lqdy
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));
    assert_eq!(
        res.allocation,
        vec![
            ("auto".to_string(), Uint128::from(50_000u128)),
            ("lqdy".to_string(), Uint128::from(149_500u128)),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ]
    );
}

#[test]
fn test_fee_adjustment() {
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        Some(Decimal::percent(2)),
    );

    // Verify initial fees
    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee.rates,
        FeeRates {
            management: Some(Decimal::percent(1)),
            performance: None,
            transaction: Some(Decimal::percent(2)),
        }
    );

    // Update fees
    let new_fee_rates = FeeRates {
        management: Some(Decimal::from_str("0.5").unwrap()),
        performance: None,
        transaction: Some(Decimal::from_str("1").unwrap()),
    };
    test_env
        .index
        .sudo_set_fees(&mut test_env.app, None, new_fee_rates.clone())
        .unwrap();

    // Verify updated fees
    let fee = test_env.index.query_fees(&mut test_env.app).unwrap();
    assert_eq!(
        fee,
        FeeManager {
            last_accrual_time: test_env.app.block_info().time,
            high_water_mark: Uint128::zero(),
            rates: new_fee_rates,
        }
    );
}

#[test]
fn test_cannot_remove_allocation_with_balance() {
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(100_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // Deposit to create balance for lqdy
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000u128, "nami"),
                coin(10_000_000u128, "auto"),
                coin(10_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // Verify lqdy balance
    let lqdy_balance = test_env
        .app
        .query_balance(&test_env.index.address.as_str(), "lqdy", false);
    assert!(lqdy_balance > Uint128::zero());

    // Attempt to remove lqdy allocation
    let res = test_env
        .index
        .sudo_remove_allocation(&mut test_env.app, "lqdy".to_string());
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Weight must be zero"));
}

#[test]
fn test_add_allocation() {
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
                coin(20_000_000_000, "eth-usdc"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(100_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // Deposit to create some shares
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![coin(10_000_000u128, "nami"), coin(10_000_000u128, "auto")],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // Add new allocation for lqdy
    let lqdy_swap = MockFin::new_app_layer(&mut test_env.app, "lqdy", "eth-usdc");
    let owner = test_env.app.api().addr_make("owner");
    lqdy_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "lqdy"),
            ],
            Decimal::from_str("1").unwrap(),
            &[1u64, 2u64, 3u64],
            Uint128::from(10_000_000_000u128),
        )
        .unwrap();

    test_env
        .index
        .sudo_add_allocation(
            &mut test_env.app,
            ("lqdy".to_string(), lqdy_swap.address.to_string()),
        )
        .unwrap();

    // Verify new allocation
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::zero()),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ]
    );

    for swap in test_env.swaps.iter() {
        test_env
            .index
            .sudo_add_allocation(
                &mut test_env.app,
                (swap.0.clone(), swap.1.address.to_string()),
            )
            .unwrap();
    }

    // Verify new allocation
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::zero()),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ]
    );
}

#[test]
fn test_add_allocation_zero_rcpt() {
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
                coin(20_000_000_000, "eth-usdc"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(100_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // Add new allocation for lqdy
    let lqdy_swap = MockFin::new_app_layer(&mut test_env.app, "lqdy", "eth-usdc");
    let owner = test_env.app.api().addr_make("owner");
    lqdy_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(100_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "lqdy"),
            ],
            Decimal::from_str("1").unwrap(),
            &[1u64, 2u64, 3u64],
            Uint128::from(10_000_000_000u128),
        )
        .unwrap();

    test_env
        .index
        .sudo_add_allocation(
            &mut test_env.app,
            ("lqdy".to_string(), lqdy_swap.address.to_string()),
        )
        .unwrap();

    // Verify new allocation
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::zero()),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ]
    );

    for swap in test_env.swaps.iter() {
        test_env
            .index
            .sudo_add_allocation(
                &mut test_env.app,
                (swap.0.clone(), swap.1.address.to_string()),
            )
            .unwrap();
    }

    // Verify new allocation everything is set to zero. Only case possible if rcpt token supply is 0
    // this can be used as security behaviour as deprecating an old contract
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::zero()),
            ("lqdy".to_string(), Uint128::zero()),
            ("nami".to_string(), Uint128::zero()),
        ]
    );
}

#[test]
fn test_remove_allocation() {
    let balances = vec![
        (
            "user",
            vec![
                coin(2_000_000_000_000u128, "nami"),
                coin(2_000_000_000_000u128, "auto"),
                coin(2_000_000_000_000u128, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000u128, "nami"),
                coin(100_000_000_000u128, "auto"),
                coin(100_000_000_000u128, "lqdy"),
                coin(500_000_000_000u128, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    let lqdy_balance = test_env
        .app
        .query_balance(&test_env.index.address.as_str(), "lqdy", false);
    assert_eq!(lqdy_balance, Uint128::zero());

    let owner = test_env.app.api().addr_make("owner");
    test_env.app.add_balance(
        &owner.to_string(),
        vec![
            coin(100_000_000_000_000u128, "eth-usdc"),
            coin(100_000_000_000_000u128, "lqdy"),
            coin(100_000_000_000_000u128, "auto"),
        ],
        true,
    );

    test_env.swaps.iter().for_each(|(denom, swap)| {
        if denom == "lqdy" || denom == "auto" {
            swap.populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(100_000_000_000u128, "eth-usdc"),
                    coin(100_000_000_000u128, denom.clone()),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(20_000_000_000u128),
            )
            .unwrap();
        }
    });

    let deposit_amounts = vec![
        coin(10_000_000u128, "nami"),
        coin(10_000_000u128, "auto"),
        coin(10_000_000u128, "lqdy"),
    ];
    let res = test_env
        .index
        .execute_deposit(&mut test_env.app, "user", deposit_amounts)
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    let reallocate_weight = Uint128::from(50_000u128);
    let res = test_env
        .index
        .sudo_reallocate(&mut test_env.app, "lqdy", "auto", reallocate_weight, None)
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/reallocate"));

    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(149_500u128)), // the price is 99$ per liquidy token when sell and 1$ when buy auto
            ("lqdy".to_string(), Uint128::from(50_000u128)),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ],
        "Unexpected allocation after first reallocate"
    );

    // Reallocate remaining lqdy weight to zero
    let res = test_env
        .index
        .sudo_reallocate(&mut test_env.app, "lqdy", "auto", Uint128::zero(), None)
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/reallocate"));

    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(199_000u128)),
            ("lqdy".to_string(), Uint128::zero()),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ],
        "Unexpected allocation after second reallocate"
    );

    test_env
        .index
        .sudo_remove_allocation(&mut test_env.app, "lqdy".to_string())
        .unwrap();

    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(
        status.allocation,
        vec![
            ("auto".to_string(), Uint128::from(199_000u128)),
            ("nami".to_string(), Uint128::from(100_000u128)),
        ],
        "Unexpected allocation after removal"
    );
}

#[test]
pub fn test_add_contract_wrong_denom() {
    let balances = vec![
        (
            "user",
            vec![
                coin(2_000_000_000_000u128, "nami"),
                coin(2_000_000_000_000u128, "auto"),
                coin(2_000_000_000_000u128, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000u128, "nami"),
                coin(100_000_000_000u128, "auto"),
                coin(100_000_000_000u128, "lqdy"),
                coin(500_000_000_000u128, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    let wrong_denom_contract = MockFin::new_app_layer(&mut test_env.app, "lqdy", "wrong_denom");

    test_env
        .index
        .sudo_add_allocation(
            &mut test_env.app,
            ("lqdy".to_string(), wrong_denom_contract.address.to_string()),
        )
        .unwrap_err();
}

#[test]
fn minimal_withdraw_user_incur_loss_of_funds() {
    // in this test we demonstrate that using to_unit_ceil
    // can cause loss of funds when withdrawing from the index
    // when the amount is minimal 1u128

    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(50u128)),
            ("auto".to_string(), Uint128::from(100u128)),
            ("lqdy".to_string(), Uint128::from(20u128)),
        ],
        None,
        Some(Decimal::percent(2)),
    );
    let rcpt_denom = format!("x/nami-index-fixed-{}-rcpt", test_env.index.address);

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(50u128, "nami"),
                coin(100u128, "auto"),
                coin(20u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "1".to_string())]));

    // Successful withdraw correct proportion
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            vec![coin(1u128, rcpt_denom.clone())],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/withdraw"));

    // In the HAL-13 commit, this assertion is commented because we only
    // send a burn event when `net > 0`.
    //
    // res.assert_event(&Event::new("burn").add_attributes(vec![("amount", "1".to_string())]));
}

#[test]
fn test_withdraw_fee_transfer() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
            ],
        ),
    ];

    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(50u128)),
            ("auto".to_string(), Uint128::from(100u128)),
            ("lqdy".to_string(), Uint128::from(20u128)),
        ],
        None,
        Some(Decimal::percent(2)),
    );
    let rcpt_denom = format!("x/nami-index-fixed-{}-rcpt", test_env.index.address);
    let user_addr = test_env.app.api().addr_make("user");
    let fee_collector_addr = test_env.app.api().addr_make("fee_collector");

    // Deposit to mint 100 receipt tokens
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(5000u128, "nami"),
                coin(10000u128, "auto"),
                coin(2000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "100".to_string())]));

    // Verify user has 100 receipt tokens
    let user_balance = test_env
        .app
        .query_balance(user_addr.as_str(), &rcpt_denom, false);
    assert_eq!(
        user_balance,
        Uint128::from(100u128),
        "User should have 100 receipt tokens"
    );

    // Withdraw 100 shares with 2% fee (expect 98 net, 2 fee)
    let withdraw_amount = coin(100u128, rcpt_denom.clone());
    let res = test_env
        .index
        .execute_withdraw(&mut test_env.app, "user", vec![withdraw_amount])
        .unwrap();

    // Verify withdraw event
    res.assert_event(
        &Event::new("wasm-nami-index-fixed/withdraw")
            .add_attributes(vec![("owner", user_addr.as_str()), ("shares", "100")]),
    );

    // Verify burn event for net shares (100 - 2% = 98)
    res.assert_event(&Event::new("burn").add_attributes(vec![("amount", "98".to_string())]));

    // Verify fee collector received 2 shares
    let fee_collector_balance =
        test_env
            .app
            .query_balance(fee_collector_addr.as_str(), &rcpt_denom, false);
    assert_eq!(
        fee_collector_balance,
        Uint128::from(2u128),
        "Fee collector should receive 2 shares"
    );

    // Verify user has no receipt tokens now
    let user_balance = test_env
        .app
        .query_balance(user_addr.as_str(), &rcpt_denom, false);
    assert_eq!(
        user_balance,
        Uint128::zero(),
        "User should not have any receipt tokens after withdrawal"
    );

    // Minimal withdrawal test (1 share)
    // Re-deposit to mint 1 receipt token
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(50u128, "nami"),
                coin(100u128, "auto"),
                coin(20u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("mint").add_attributes(vec![("amount", "1".to_string())]));

    // Withdraw 1 share (expected net = 0, fee = 1 due to rounding)
    let res = test_env
        .index
        .execute_withdraw(
            &mut test_env.app,
            "user",
            vec![coin(1u128, rcpt_denom.clone())],
        )
        .unwrap();

    // No burn event expected since net is 0
    assert!(
        !res.has_event(&Event::new("burn")),
        "No burn event should be emitted when net is zero"
    );

    // Verify fee collector received 1 share
    let fee_collector_balance =
        test_env
            .app
            .query_balance(fee_collector_addr.as_str(), &rcpt_denom, false);
    assert_eq!(
        fee_collector_balance,
        Uint128::from(3u128), // 2 from previous + 1 from this withdrawal
        "Fee collector should receive 1 additional share for minimal withdrawal"
    );
}

#[test]
fn test_callback_unauthorized_sender() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
                coin(10_000_000_000, "eth-usdc"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(100_000_000_000, "eth-usdc"),
            ],
        ),
    ];

    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        None,
        None,
    );

    let unauthorized_contract = test_env.app.api().addr_make("malicious_contract");

    // Get the swap contract address for "auto"
    let auto_swap = test_env
        .swaps
        .iter()
        .find(|(denom, _)| denom == "auto")
        .map(|(_, swap)| swap.address.clone())
        .expect("Auto swap contract not found");

    // Create a callback message
    let callback_type = CallbackType::AfterReallocate {
        swap_to: auto_swap.clone(),
        amount: Uint128::from(128u128),
        min_return: Some(Uint128::from(128u128)),
    };
    let callback_msg = ExecuteMsg::Callback(CallbackMsg {
        data: to_json_binary(&callback_type).unwrap(),
        callback: CallbackData(to_json_binary(&callback_type).unwrap()),
    });

    // Execute callback with unauthorized sender
    let res = test_env.app.app.execute_contract(
        unauthorized_contract,
        test_env.index.address.clone(),
        &callback_msg,
        &[],
    );

    // Verify the callback fails with Unauthorized error
    assert!(
        res.is_err(),
        "Callback from unauthorized sender should fail"
    );
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");
}

#[test]
fn test_callback_empty_allocations() {
    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "lqdy"),
                coin(10_000_000_000, "eth-usdc"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(100_000_000_000, "eth-usdc"),
            ],
        ),
    ];

    let mut test_env = index::setup(balances, "eth-usdc".to_string(), vec![], None, None);

    let mock_contract = test_env.app.api().addr_make("mock_contract");

    let callback_type = CallbackType::AfterReallocate {
        swap_to: mock_contract.clone(),
        amount: Uint128::from(128u128),
        min_return: Some(Uint128::from(128u128)),
    };
    let callback_msg = ExecuteMsg::Callback(CallbackMsg {
        data: to_json_binary(&callback_type).unwrap(),
        callback: CallbackData(to_json_binary(&callback_type).unwrap()),
    });

    // Execute callback with mock sender
    let res = test_env.app.app.execute_contract(
        mock_contract,
        test_env.index.address.clone(),
        &callback_msg,
        &[],
    );

    // Verify the callback fails with Unauthorized error
    assert!(res.is_err(), "Callback with empty allocations should fail");
    assert_eq!(res.unwrap_err().root_cause().to_string(), "Unauthorized");
}

#[test]
fn isolating_reallocate_funds() {
    // Scope of the test is to demonstrate that reallocate funds
    // only uses the correct amount of funds for the reallocation
    // excluding airdrops/erroneus sends to the index

    // Initialize user balances
    let balances = vec![
        (
            "user",
            vec![
                coin(20_000_000_000, "nami"),
                coin(20_000_000_000, "auto"),
                coin(20_000_000_000, "lqdy"),
            ],
        ),
        (
            "owner",
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(100_000_000_000, "lqdy"),
                coin(500_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(100_000u128)),
            ("auto".to_string(), Uint128::from(100_000u128)),
            ("lqdy".to_string(), Uint128::from(100_000u128)),
        ],
        Some(Decimal::percent(1)),
        None,
    );

    // send some funds to the index normal bank send
    test_env.app.add_balance(
        test_env.index.address.as_str(),
        vec![coin(10_000_000u128, "eth-usdc")],
        false,
    );

    // Successful deposit correct proportion
    let res = test_env
        .index
        .execute_deposit(
            &mut test_env.app,
            "user",
            vec![
                coin(10_000_000u128, "nami"),
                coin(10_000_000u128, "auto"),
                coin(10_000_000u128, "lqdy"),
            ],
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/deposit"));

    // query status to see the inflation Zero because no execution
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));

    // populate orderbooks
    let owner = test_env.app.api().addr_make("owner");
    test_env.swaps.iter().for_each(|swap| {
        swap.1
            .populate_orderbook(
                &mut test_env.app,
                &owner,
                vec![
                    coin(100_000_000_000, "eth-usdc"),
                    coin(100_000_000_000, swap.0.clone()),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(10_000_000_000u128),
            )
            .unwrap();
    });

    //  try reallocation
    let res = test_env
        .index
        .sudo_reallocate(
            &mut test_env.app,
            "auto",
            "lqdy",
            Uint128::from(50_000u128),
            None,
        )
        .unwrap();
    res.assert_event(&Event::new("wasm-nami-index-fixed/reallocate"));

    // query status to see the new allocation
    // selling auto for usdc price 0.99 gets 495_000 usdc
    // selling usdc for lqdy price 1.00 gets 495_000 lqdy
    let res = test_env.index.query_status(&mut test_env.app).unwrap();
    assert_eq!(res.total_shares, Uint128::from(100u128));
    assert_eq!(
        res.allocation,
        vec![
            ("auto".to_string(), Uint128::from(50_000u128)),
            ("lqdy".to_string(), Uint128::from(149_500u128)),
            ("nami".to_string(), Uint128::from(100_000u128))
        ]
    );

    // query balance index to see that the eth-usdc previosly sent are still there
    let res = test_env
        .app
        .query_balance(&test_env.index.address.as_str(), "eth-usdc", false);
    assert_eq!(res, Uint128::from(10_000_000u128));
}

#[test]
fn test_add_allocation_denom_validation() {
    // Initialize user balances
    let balances = vec![
        (
            "owner",
            vec![
                coin(200_000_000_000, "nami"),
                coin(100_000_000_000, "auto"),
                coin(300_000_000_000, "eth-usdc"),
                coin(100_000_000_000, "invalid-denom"),
                coin(100_000_000_000, "wrong-denom"),
            ],
        ),
        (
            "user",
            vec![
                coin(10_000_000_000, "nami"),
                coin(10_000_000_000, "auto"),
                coin(10_000_000_000, "eth-usdc"),
            ],
        ),
    ];
    let mut test_env = index::setup(
        balances,
        "eth-usdc".to_string(),
        vec![
            ("nami".to_string(), Uint128::from(50u128)),
            ("auto".to_string(), Uint128::from(100u128)),
        ],
        None,
        None,
    );
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

    // Valid pair (quote_denom = eth-usdc, denom = nami)
    let nami_swap = test_env
        .swaps
        .iter()
        .find(|(denom, _)| denom == "nami")
        .unwrap();
    test_env
        .index
        .sudo_add_allocation(
            &mut test_env.app,
            ("nami".to_string(), nami_swap.1.address.to_string()),
        )
        .unwrap();
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert!(
        status.allocation.iter().any(|(denom, _)| denom == "nami"),
        "Nami allocation not found"
    );

    // Valid flipped pair (quote_denom = eth-usdc, denom = nami, swap: quote = nami, base = eth-usdc)
    let flipped_swap =
        nami_rs_testing::mock_fin::MockFin::new_app_layer(&mut test_env.app, "eth-usdc", "nami");
    flipped_swap
        .populate_orderbook(
            &mut test_env.app,
            &owner,
            vec![
                coin(100_000_000_000, "nami"),
                coin(100_000_000_000, "eth-usdc"),
            ],
            Decimal::from_str("100").unwrap(),
            &[1u64, 2u64, 3u64],
            Uint128::from(1_000_000_000u128),
        )
        .unwrap();
    test_env
        .index
        .sudo_add_allocation(
            &mut test_env.app,
            ("nami".to_string(), flipped_swap.address.to_string()),
        )
        .unwrap();
    let status = test_env.index.query_status(&mut test_env.app).unwrap();
    assert!(
        status.allocation.iter().any(|(denom, _)| denom == "nami"),
        "Nami allocation (flipped) not found"
    );

    // Invalid pair
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
        .sudo_add_allocation(
            &mut test_env.app,
            (
                "invalid-denom".to_string(),
                invalid_swap.address.to_string(),
            ),
        )
        .unwrap_err();
    assert_eq!(res.root_cause().to_string(), "Invalid denom pair");
}
