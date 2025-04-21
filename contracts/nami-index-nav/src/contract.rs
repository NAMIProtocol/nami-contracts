#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
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
    let aum_fees_msg = rcpt.mint_msg(aum_fee, config.fee_collector.clone());
    match msg {
        ExecuteMsg::Deposit {} => {
            let amount = must_pay(&info, &config.base_denom)?;
            let nav = vault.nav(&deps.querier, Some(amount), rcpt.supply(deps.querier)?)?;
            let minted = Decimal::from_ratio(amount, Uint128::one())
                .checked_div(nav)?
                .to_uint_floor();
            let response =
                Response::new().add_event(event_deposit(info.sender.clone(), amount, minted));
            Ok(response
                .add_message(rcpt.mint_msg(minted, info.sender))
                .add_message(aum_fees_msg))
        }
        ExecuteMsg::Withdraw {} => {
            let amount = must_pay(&info, rcpt.denom().as_str())?;
            let (net, burn_fee) = FeeCollector::exit_fee(deps.storage, amount)?;
            let nav = vault.nav(&deps.querier, None, rcpt.supply(deps.querier)?)?;
            let withdraw_amount = Decimal::from_ratio(net, Uint128::one())
                .checked_mul(nav)?
                .to_uint_floor();
            let withdraw_msgs =
                vault.withdraw(&deps.querier, withdraw_amount, info.sender.clone())?;
            let response = Response::new().add_event(event_withdraw(
                info.sender.clone(),
                amount,
                withdraw_amount,
            ));
            Ok(response
                .add_message(rcpt.burn_msg(amount))
                .add_message(rcpt.mint_msg(burn_fee, config.fee_collector))
                .add_message(aum_fees_msg)
                .add_messages(withdraw_msgs))
        }
        ExecuteMsg::Run {} => {
            nonpayable(&info)?;
            let rebalance_msgs = vault.rebalance(&deps.querier)?;
            Ok(Response::new()
                .add_event(event_run(info.sender.clone()))
                .add_messages(rebalance_msgs))
        }
    }
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
mod tests {}
