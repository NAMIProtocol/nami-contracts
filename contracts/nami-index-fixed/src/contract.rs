#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_utils::must_pay;
use nami_rs::index_fixed::{CallbackType, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg};
use rujira_rs::TokenFactory;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::{event_deposit, event_reallocate, event_withdraw};
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
    Vault::init(deps.storage, deps.api, msg.target_denoms.clone())?;

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
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    Vault::rebalance(
        deps.storage,
        &env,
        &deps.querier,
        rcpt.supply(deps.querier)?,
    )?;
    let aum_fee = FeeCollector::aum_fee(deps.storage, &env, rcpt.supply(deps.querier)?)?;
    let aum_fees_msg = rcpt.mint_msg(aum_fee, config.fee_collector.clone());
    match msg {
        ExecuteMsg::Deposit {} => {
            let amount = Vault::deposit(deps.storage, info.funds.clone())?;
            let response = Response::new().add_event(event_deposit(
                info.sender.clone(),
                info.funds.clone(),
                amount,
            ));
            Ok(response
                .add_message(rcpt.mint_msg(amount, info.sender))
                .add_message(aum_fees_msg))
        }
        ExecuteMsg::Withdraw {} => {
            let amount = must_pay(&info, rcpt.denom().as_str())?;
            let (net, burn_fee) = FeeCollector::exit_fee(deps.storage, amount)?;

            let withdraw_funds = Vault::withdraw(deps.storage, net)?;
            let response = Response::new().add_event(event_withdraw(
                info.sender.clone(),
                withdraw_funds.clone(),
                amount,
            ));
            let send_msg = BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: withdraw_funds,
            };
            Ok(response
                .add_message(rcpt.burn_msg(amount))
                .add_message(rcpt.mint_msg(burn_fee, config.fee_collector))
                .add_message(aum_fees_msg)
                .add_message(send_msg))
        }
        ExecuteMsg::Callback(cb) => {
            let callback_type: CallbackType = cb.deserialize_callback()?;
            match callback_type {
                CallbackType::AfterReallocate { swap_to } => {
                    let msg = Vault::after_reallocate(
                        &env,
                        deps.querier,
                        swap_to,
                        config.base_denom.clone(),
                    )?;
                    Ok(Response::new().add_message(msg))
                }
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    let total_supply = rcpt.supply(deps.querier)?;
    Vault::rebalance(deps.storage, &env, &deps.querier, total_supply)?;
    match msg {
        SudoMsg::Reallocate { from, to, weight } => {
            let msg =
                Vault::reallocate(deps.storage, from.clone(), to.clone(), weight, total_supply)?;
            let response = Response::new().add_event(event_reallocate(from, to, weight));
            Ok(response.add_message(msg))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> Result<Binary, ContractError> {
    Ok(to_json_binary(&())?)
}

#[cfg(test)]
mod tests {}
