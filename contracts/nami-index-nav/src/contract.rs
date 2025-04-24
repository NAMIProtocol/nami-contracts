#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError, Uint128,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, nonpayable};
use nami_rs::index_nav::{ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg};
use rujira_rs::TokenFactory;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::{event_deposit, event_run, event_withdraw};
use crate::fee_collector::FeeCollector;
use crate::vault::Vault;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::new(deps.api, msg.clone())?;
    config.save(deps.storage)?;
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    FeeCollector::init(deps.storage, &env, msg.fees)?;
    Ok(Response::default().add_message(rcpt.create_msg(msg.receipt)))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let config = Config::load(deps.storage)?;
    let vault = Vault::new(&env, &config)?;
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    let aum_fee = FeeCollector::aum_fee(deps.storage, &env, rcpt.supply(deps.querier)?)?;
    let mut response = Response::new();
    match msg {
        ExecuteMsg::Deposit {} => {
            let amount = must_pay(&info, &config.base_denom)?;
            let nav = vault.nav(&deps.querier, Some(amount), rcpt.supply(deps.querier)?)?;
            let minted = Decimal::from_ratio(amount, Uint128::one())
                .checked_div(nav)?
                .to_uint_floor();
            if minted.is_zero() {
                return Err(ContractError::Std(StdError::generic_err(
                    "Minted amount is zero",
                )));
            }
            response = response
                .add_event(event_deposit(info.sender.clone(), amount, minted))
                .add_message(rcpt.mint_msg(minted, info.sender));
        }
        ExecuteMsg::Withdraw {} => {
            let amount = must_pay(&info, rcpt.denom().as_str())?;
            let (net, burn_fee) = FeeCollector::exit_fee(deps.storage, amount)?;
            let nav = vault.nav(&deps.querier, None, rcpt.supply(deps.querier)?)?;
            let withdraw_amount = Decimal::from_ratio(net, Uint128::one())
                .checked_mul(nav)?
                .to_uint_floor();
            response = response
                .add_event(event_withdraw(info.sender.clone(), withdraw_amount, amount))
                .add_message(rcpt.burn_msg(amount))
                .add_messages(vault.withdraw(
                    &deps.querier,
                    withdraw_amount,
                    info.sender.clone(),
                )?);

            if burn_fee.gt(&Uint128::zero()) {
                response = response.add_message(rcpt.mint_msg(burn_fee, config.fee_collector.clone()));
            }
        }
        ExecuteMsg::Run {} => {
            nonpayable(&info)?;
            let rebalance_msgs = vault.rebalance(&deps.querier)?;
            response = response
                .add_event(event_run(info.sender.clone()))
                .add_messages(rebalance_msgs);
        }
    }
    if !aum_fee.is_zero() {
        response = response.add_message(rcpt.mint_msg(aum_fee, config.fee_collector));
    }
    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(_deps: DepsMut, _env: Env, _msg: SudoMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> Result<Binary, ContractError> {
    Ok(to_json_binary(&())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::setup;
    use cosmwasm_std::{coins, Event, Uint128};
    use cw_multi_test::Executor;
    use rujira_rs_testing::mock_rujira_app;

    #[test]
    fn test_instantiation() {
        let mut app = mock_rujira_app();
        let owner = app.api().addr_make("owner");

        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &owner, coins(10_000_000_000, "eth.usdc"))
                .unwrap();
        });

        let index_nav = setup::index_nav(
            &mut app,
            "eth.usdc".to_string(),
            vec!["btc.btc".to_string()],
        );

        let receipt_denom = format!("x/nami-index-{}-rcpt", index_nav.address);
        let balance = app
            .wrap()
            .query_balance(&index_nav.address, &receipt_denom)
            .unwrap();
        assert_eq!(balance.amount, Uint128::zero());

        // Verify initial contract balances
        let usdc_balance = app
            .wrap()
            .query_balance(&index_nav.address, "eth.usdc")
            .unwrap()
            .amount;
        let btc_balance = app
            .wrap()
            .query_balance(&index_nav.address, "btc.btc")
            .unwrap()
            .amount;
        assert_eq!(usdc_balance, Uint128::zero());
        assert_eq!(btc_balance, Uint128::zero());
    }

    #[test]
    fn lifecycle() {
        let mut app = mock_rujira_app();
        let user = app.api().addr_make("user");
        let fee_collector = app.api().addr_make("fee_collector");

        // Initialize user balances
        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &user, coins(10_000_000_000, "eth.usdc"))
                .unwrap();
        });

        let index_nav = setup::index_nav(
            &mut app,
            "eth.usdc".to_string(),
            vec!["btc.btc".to_string()],
        );
        let rcpt_denom = format!("x/nami-index-{}-rcpt", index_nav.address);

        // Successful deposit
        let deposit_amount = Uint128::from(5_000_000u128);
        let res = app
            .execute_contract(
                user.clone(),
                index_nav.address.clone(),
                &ExecuteMsg::Deposit {},
                &coins(deposit_amount.u128(), "eth.usdc"),
            )
            .unwrap();

        // Minted = 5000000 / 1.00 = 5000000
        res.assert_event(
            &Event::new("wasm-nami-index-nav/deposit").add_attributes(vec![
                ("owner", user.as_str()),
                ("amount", deposit_amount.to_string().as_str()),
                ("minted", "5000000"),
            ]),
        );

        res.assert_event(&Event::new("mint").add_attributes(vec![
            ("amount", "5000000"),
            ("denom", rcpt_denom.as_str()),
            ("recipient", user.as_str()),
        ]));

        // Check user receipt token balance
        let rcpt_balance = app
            .wrap()
            .query_balance(user.clone(), rcpt_denom.clone())
            .unwrap()
            .amount;
        assert_eq!(rcpt_balance, Uint128::from(5_000_000u128));

        // Check contract balances
        let usdc_balance = app
            .wrap()
            .query_balance(&index_nav.address, "eth.usdc")
            .unwrap()
            .amount;
        let btc_balance = app
            .wrap()
            .query_balance(&index_nav.address, "btc.btc")
            .unwrap()
            .amount;
        assert_eq!(usdc_balance, Uint128::from(5_000_000u128));
        assert_eq!(btc_balance, Uint128::zero());

        app.update_block(|block| block.time = block.time.plus_seconds(60));

        // Successful withdraw
        let withdraw_rcpt_amount = Uint128::from(1_000_000u128);
        let res = app
            .execute_contract(
                user.clone(),
                index_nav.address.clone(),
                &ExecuteMsg::Withdraw {},
                &coins(withdraw_rcpt_amount.u128(), rcpt_denom.clone()),
            )
            .unwrap();

        // nav = (5M * 1.00) / 5000000 = 1.00
        // fees = 1000000 * 1% = 10000
        // shares = 1000000 - 10000 = 990000
        // withdraw amount = 990000 * 1.001 = 990990
        res.assert_event(
            &Event::new("wasm-nami-index-nav/withdraw").add_attributes(vec![
                ("owner", user.as_str()),
                ("amount", "990990"),
                ("shares", "1000000"),
            ]),
        );

        res.assert_event(&Event::new("burn").add_attributes(vec![
            ("amount", withdraw_rcpt_amount.to_string().as_str()),
            ("denom", rcpt_denom.as_str()),
        ]));

        // Verify user usdc balance
        let usdc_balance = app
            .wrap()
            .query_balance(user.clone(), "eth.usdc")
            .unwrap()
            .amount;
        assert_eq!(
            usdc_balance,
            Uint128::from(10_000_000_000u128 - 5_000_000u128 + 990_990u128)
        );

        // Verify fee collector balance (10000 shares: 1% of 1000000)
        let fee_balance = app
            .wrap()
            .query_balance(&fee_collector, &rcpt_denom)
            .unwrap()
            .amount;
        assert_eq!(fee_balance, Uint128::from(10_000u128));

        // Test rebalance, etc
    }
}
