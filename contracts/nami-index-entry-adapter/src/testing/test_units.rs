use std::str::FromStr;

use crate::testing::env;
use cosmwasm_std::{coin, coins, Decimal, Uint128};
use nami_rs::index_entry_adapter::SwapEntry;

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
                coin(10_000_000_000, "eth-usdc"),
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
    let mut test_env = env::setup(
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

    // populate swap mocks so that fair price is 1 for everyone
    let owner = test_env.app.api().addr_make("owner");
    test_env.swaps.iter().for_each(|(denom, swap_mock)| {
        swap_mock
            .populate_orderbook(
                &mut test_env.app,
                &owner.clone(),
                vec![
                    coin(100_000_000_000, "eth-usdc"),
                    coin(100_000_000_000, denom.clone()),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(1_000_000_000u128),
            )
            .unwrap();
    });

    // Successful deposit correct proportion
    let res = test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(170000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(50000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(100000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(20000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap();
    let rcpt_denom = format!("x/nami-index-fixed-{}-rcpt", test_env.index.address);

    // Successful withdraw correct proportion
    let res = test_env
        .entry_adapter
        .execute_withdraw(
            &mut test_env.app,
            "user",
            coins(10u128, rcpt_denom.clone()),
            test_env.index.address.to_string(),
            None,
        )
        .unwrap();

    // Sudo Tests remove swap contract
    let res = test_env
        .entry_adapter
        .sudo_remove_swap_contract(&mut test_env.app, "auto".to_string())
        .unwrap();

    //  try deposit should fail
    // Successful deposit correct proportion
    let res = test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(170000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(50000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(100000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(20000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap_err();

    // add auto back
    let auto_swap = test_env
        .swaps
        .iter()
        .find(|(denom, _)| denom == "auto")
        .unwrap();
    test_env
        .entry_adapter
        .sudo_add_swap_contract(
            &mut test_env.app,
            "auto".to_string(),
            auto_swap.1.address.to_string(),
        )
        .unwrap();

    // try deposit should work
    let res = test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(170000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(50000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(100000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(20000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap();

    // queries
    let res = test_env
        .entry_adapter
        .query_swap_contracts(&mut test_env.app)
        .unwrap();
    assert_eq!(res.swap_contracts.len(), 3);

    let res = test_env
        .entry_adapter
        .query_swap_contract(&mut test_env.app, "auto".to_string())
        .unwrap();
    assert_eq!(res.contract, auto_swap.1.address.to_string());

    let res = test_env
        .entry_adapter
        .query_config(&mut test_env.app)
        .unwrap();
    assert_eq!(res.quote_denom, "eth-usdc".to_string());

    // try deposit with wrong data should fail
    test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(17000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(50000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(100000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(20000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap_err();

    // try deposit with remaining tokens should succed and send tokens back
    let res = test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(150000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(40000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(90000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(20000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap();
}

#[test]
fn test_swap_with_extra_tokens() {
    // Initialize user balances
    let balances = vec![
        ("user", vec![coin(10_000_000_000, "eth-usdc")]),
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
    let mut test_env = env::setup(
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

    // populate swap mocks so that fair price is 1 for everyone
    let owner = test_env.app.api().addr_make("owner");
    test_env.swaps.iter().for_each(|(denom, swap_mock)| {
        swap_mock
            .populate_orderbook(
                &mut test_env.app,
                &owner.clone(),
                vec![
                    coin(100_000_000_000, "eth-usdc"),
                    coin(100_000_000_000, denom.clone()),
                ],
                Decimal::from_str("100").unwrap(),
                &[1u64, 2u64, 3u64],
                Uint128::from(1_000_000_000u128),
            )
            .unwrap();
    });

    // Successful deposit swap generates extra tokens send back tokens
    let res = test_env
        .entry_adapter
        .execute_deposit(
            &mut test_env.app,
            "user",
            coins(154000u128, "eth-usdc"),
            test_env.index.address.to_string(),
            vec![
                SwapEntry {
                    amount: Uint128::from(45000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(90000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(19000u128), // 1000 extra
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        )
        .unwrap();

    // assert balance of user
    // 10_000_000_000 - 154000u128 + (1000 extra swapped in auto and then swaped back price per orderbook is 0.99)
    let lqdy_balance = test_env.app.query_balance("user", "eth-usdc", true);
    assert_eq!(
        lqdy_balance,
        Uint128::from(10_000_000_000u128 - 154000u128 + 990u128)
    );
}
