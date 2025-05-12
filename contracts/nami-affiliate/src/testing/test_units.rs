use crate::testing::index;
use cosmwasm_std::{coin, coins, to_json_binary, Decimal, Event, Uint128};
use cw_multi_test::Executor;
use nami_rs::affiliate::ExecuteMsg;
use nami_rs::index_entry_adapter::SwapEntry;
use nami_rs::{index_entry_adapter, index_nav};
use std::ops::DerefMut;
use std::str::FromStr;

use super::index_fixed;

#[test]
fn lifecycle() {
    // Initialize user balances
    let balances = vec![
        ("sender", vec![coin(10_000_000, "eth-usdc")]),
        ("affiliate", vec![coin(0, "eth-usdc")]),
        ("someone", vec![coin(0, "eth-usdc")]),
        ("fee_collector", vec![coin(0, "eth-usdc")]),
    ];
    let mut test_env = index::setup(balances);

    let affiliate_addr = test_env.app.api().addr_make("affiliate").to_string();

    // Execute with affiliate (10% fee), forwarding to nami-index-nav Deposit
    let funds = vec![coin(1000, "eth-usdc")];
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.target_contract.clone(),
        msg: to_json_binary(&index_nav::ExecuteMsg::Deposit {}).unwrap(),
        affiliate: Some((affiliate_addr.clone(), 1000u16)),
    };
    let res = test_env
        .affiliate
        .execute(&mut test_env.app, "sender", msg, &funds)
        .unwrap();

    // Verify events
    res.assert_event(
        &Event::new("wasm-nami-affiliate-execute").add_attributes(vec![
            ("contract_addr", test_env.target_contract.as_str()),
            ("affiliate", affiliate_addr.as_str()),
            ("bps", "1000"),
        ]),
    );

    // Check balances
    let affiliate_balance = test_env.app.query_balance("affiliate", "eth-usdc", true);
    assert_eq!(affiliate_balance, Uint128::new(100)); // 10% of 1000

    let target_balance = test_env
        .app
        .query_balance(&test_env.target_contract, "eth-usdc", false);
    assert_eq!(target_balance, Uint128::new(900)); // 90% of 1000

    let contract_balance =
        test_env
            .app
            .query_balance(&test_env.affiliate.address.as_str(), "eth-usdc", false);
    assert_eq!(contract_balance, Uint128::zero()); // Empty after chained Send

    let sender_balance = test_env.app.query_balance("sender", "eth-usdc", true);
    assert_eq!(sender_balance, Uint128::new(10_000_000 - 1000));

    let rcpt_balance = test_env.app.query_balance(
        "sender",
        format!("x/nami-index-{}-rcpt", test_env.target_contract).as_str(),
        true,
    );
    assert_eq!(rcpt_balance, Uint128::new(900));

    // Fund contract and execute Send
    test_env.app.add_balance(
        &test_env.affiliate.address.as_str(),
        vec![coin(1000, "eth-usdc")],
        false,
    );

    let someone_addr = test_env.app.api().addr_make("someone").to_string();

    let msg = ExecuteMsg::Send {
        sender: someone_addr.clone(),
    };
    let _res = test_env
        .app
        .deref_mut()
        .execute_contract(
            test_env.affiliate.address.clone(),
            test_env.affiliate.address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Verify recipient balance
    let recipient_balance = test_env.app.query_balance("someone", "eth-usdc", true);
    assert_eq!(recipient_balance, Uint128::new(1000));

    // Check contract balance (should be empty)
    let contract_balance =
        test_env
            .app
            .query_balance(&test_env.affiliate.address.as_str(), "eth-usdc", false);
    assert_eq!(contract_balance, Uint128::zero());

    // Execute without affiliate
    let funds = vec![coin(1000, "eth-usdc")];
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.target_contract.clone(),
        msg: to_json_binary(&index_nav::ExecuteMsg::Deposit {}).unwrap(),
        affiliate: None,
    };
    let res = test_env
        .affiliate
        .execute(&mut test_env.app, "sender", msg, &funds)
        .unwrap();

    // Verify events
    res.assert_event(
        &Event::new("wasm-nami-affiliate-execute").add_attributes(vec![
            ("contract_addr", test_env.target_contract.as_str()),
            ("affiliate", ""),
            ("bps", ""),
        ]),
    );

    // Check balances
    let target_balance = test_env
        .app
        .query_balance(&test_env.target_contract, "eth-usdc", false);
    assert_eq!(target_balance, Uint128::new(1900)); // 900 + 1000

    let sender_balance = test_env.app.query_balance("sender", "eth-usdc", true);
    assert_eq!(sender_balance, Uint128::new(10_000_000 - 2000));

    let funds = vec![coin(1000, "eth-usdc")];
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.target_contract.clone(),
        msg: to_json_binary(&index_nav::ExecuteMsg::Deposit {}).unwrap(),
        affiliate: Some((affiliate_addr.clone(), 10100u16)),
    };
    test_env
        .affiliate
        .execute(&mut test_env.app, "sender", msg, &funds)
        .unwrap_err();

    test_env
        .affiliate
        .execute_send(&mut test_env.app, "sender", &funds)
        .unwrap_err();
}

#[test]
fn base_test() {
    // Initialize user balances
    let balances = vec![
        ("sender", vec![coin(10_000_000, "eth-usdc")]),
        ("affiliate", vec![coin(0, "eth-usdc")]),
        ("fee_collector", vec![coin(0, "eth-usdc")]),
    ];
    let mut test_env = index::setup(balances);

    let affiliate_addr = test_env.app.api().addr_make("affiliate").to_string();

    // Execute with affiliate (10% fee)
    let funds = vec![coin(1000, "eth-usdc")];
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.target_contract.clone(),
        msg: to_json_binary(&index_nav::ExecuteMsg::Deposit {}).unwrap(),
        affiliate: Some((affiliate_addr.clone(), 1000u16)),
    };
    let res = test_env
        .affiliate
        .execute(&mut test_env.app, "sender", msg, &funds)
        .unwrap();

    // Verify events
    res.assert_event(
        &Event::new("wasm-nami-affiliate-execute").add_attributes(vec![
            ("contract_addr", test_env.target_contract.as_str()),
            ("affiliate", affiliate_addr.as_str()),
            ("bps", "1000"),
        ]),
    );

    // Check balances
    let affiliate_balance = test_env.app.query_balance("affiliate", "eth-usdc", true);
    assert_eq!(affiliate_balance, Uint128::new(100)); // 10% of 1000

    let target_balance = test_env
        .app
        .query_balance(&test_env.target_contract, "eth-usdc", false);
    assert_eq!(target_balance, Uint128::new(900)); // 90% of 1000

    let contract_balance =
        test_env
            .app
            .query_balance(&test_env.affiliate.address.as_str(), "eth-usdc", false);
    assert_eq!(contract_balance, Uint128::zero()); // Empty after Send

    let sender_balance = test_env.app.query_balance("sender", "eth-usdc", true);
    assert_eq!(sender_balance, Uint128::new(10_000_000 - 1000));
}

#[test]
fn lifecycle_index_fixed() {
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
    let mut test_env = index_fixed::setup(
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

    // populate orderbooks
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

    let affiliate_addr = test_env.app.api().addr_make("affiliate").to_string();

    // Execute with affiliate (10% fee)
    // make sure to build the swaps message using the net amount after fee is subtracted
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.entry_adapter.address.clone().to_string(),
        msg: to_json_binary(&index_entry_adapter::ExecuteMsg::Deposit {
            index: test_env.index.address.clone().to_string(),
            swaps: vec![
                SwapEntry {
                    amount: Uint128::from(45_000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(90_000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(18_000u128),
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        })
        .unwrap(),
        affiliate: Some((affiliate_addr.clone(), 1000u16)),
    };
    let res = test_env
        .affiliate
        .execute(&mut test_env.app, "user", msg, &coins(170_000, "eth-usdc"))
        .unwrap();

    // Verify events
    res.assert_event(
        &Event::new("wasm-nami-affiliate-execute").add_attributes(vec![
            ("contract_addr", test_env.entry_adapter.address.as_str()),
            ("affiliate", affiliate_addr.as_str()),
            ("bps", "1000"),
        ]),
    );

    // Check balances
    let affiliate_balance = test_env.app.query_balance("affiliate", "eth-usdc", true);
    assert_eq!(affiliate_balance, Uint128::new(17_000)); // 10% of 170_000

    let rcpt_denom = format!("x/nami-index-{}-rcpt", test_env.index.address);
    let user_balance = test_env
        .app
        .query_balance("user", &rcpt_denom.clone(), true);
    assert_eq!(user_balance, Uint128::new(9)); // 9 index tokens

    // Test with rest after swaps, make sure the affiliate sends everything back to the user
    // no funds stuck
    let msg = ExecuteMsg::Execute {
        contract_addr: test_env.entry_adapter.address.clone().to_string(),
        msg: to_json_binary(&index_entry_adapter::ExecuteMsg::Deposit {
            index: test_env.index.address.clone().to_string(),
            swaps: vec![
                SwapEntry {
                    amount: Uint128::from(45_000u128),
                    denom: "nami".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(90_000u128),
                    denom: "auto".to_string(),
                    min_return: None,
                },
                SwapEntry {
                    amount: Uint128::from(27_000u128), //extra 10_000
                    denom: "lqdy".to_string(),
                    min_return: None,
                },
            ],
        })
        .unwrap(),
        affiliate: Some((affiliate_addr.clone(), 1000u16)),
    };
    let res = test_env
        .affiliate
        .execute(&mut test_env.app, "user", msg, &coins(180_000, "eth-usdc"))
        .unwrap();

    // Verify events
    res.assert_event(
        &Event::new("wasm-nami-affiliate-execute").add_attributes(vec![
            ("contract_addr", test_env.entry_adapter.address.as_str()),
            ("affiliate", affiliate_addr.as_str()),
            ("bps", "1000"),
        ]),
    );

    // Check balances
    let affiliate_balance = test_env.app.query_balance("affiliate", "eth-usdc", true);
    assert_eq!(affiliate_balance, Uint128::new(35_000)); // 10% of 180_000 + 17_000 (old)

    let rcpt_denom = format!("x/nami-index-{}-rcpt", test_env.index.address);
    let user_balance = test_env
        .app
        .query_balance("user", &rcpt_denom.clone(), true);
    assert_eq!(user_balance, Uint128::new(18)); // 9 index tokens + 9 index tokens from previous deposit

    // assert user balance usdc
    // 10_000_000_000 - 170_000 - 180_000 + 9_000 * 0.99 = 9_999_658_910
    let user_balance = test_env.app.query_balance("user", "eth-usdc", true);
    assert_eq!(user_balance, Uint128::new(9_999_658_910));
}
