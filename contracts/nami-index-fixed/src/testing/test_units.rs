use std::str::FromStr;

use crate::testing::index;
use cosmwasm_std::{coin, Decimal, Event, Uint128};
use nami_rs::FeeManager;
use nami_rs::FeeRates;
use nami_rs_testing::mock_fin::MockFin;

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
    let rcpt_denom = format!("x/nami-index-{}-rcpt", test_env.index.address);

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
    let rcpt_denom = format!("x/nami-index-{}-rcpt", test_env.index.address);

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
            ("nami".to_string(), Uint128::from(99009900u128))
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
            ("nami".to_string(), Uint128::from(99502487u128))
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
            ("nami".to_string(), Uint128::from(100_000u128))
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
        Some(Decimal::percent(2))
    );
    let rcpt_denom = format!("x/nami-index-{}-rcpt", test_env.index.address);

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