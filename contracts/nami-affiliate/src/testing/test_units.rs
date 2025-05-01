use crate::testing::index;
use cosmwasm_std::{coin, to_json_binary, Event, Uint128};
use cw_multi_test::Executor;
use nami_rs::affiliate::ExecuteMsg;
use nami_rs::index_nav;
use std::ops::DerefMut;

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
