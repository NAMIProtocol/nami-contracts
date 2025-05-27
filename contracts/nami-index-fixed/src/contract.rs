#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_storage_plus::Item;
use cw_utils::must_pay;
use nami_rs::index_fixed::{
    CallbackType, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg,
};
use nami_rs::FeeManager;
use rujira_rs::fin::{self, SwapRequest};
use rujira_rs::TokenFactory;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::{event_deposit, event_reallocate, event_withdraw};
use crate::vault::Vault;

static FEE_MANAGER: Item<FeeManager> = Item::new("fee_manager");

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
    FEE_MANAGER.save(deps.storage, &FeeManager::new(msg.fees, env.block.time)?)?;
    let vault = Vault::new(deps.api, &deps.querier, env.contract.address);
    vault.init(
        deps.storage,
        msg.target_allocations.clone(),
        &msg.quote_denom,
    )?;

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
    let mut fee_manager = FEE_MANAGER.load(deps.storage)?;
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    let vault = Vault::new(deps.api, &deps.querier, env.contract.address.clone());
    let aum_fee = fee_manager.aum_fee(env.block.time, rcpt.supply(deps.querier)?)?;
    let mut response = Response::new();
    match msg {
        ExecuteMsg::Deposit {} => {
            let amount = Vault::deposit(deps.storage, info.funds.clone())?;
            response = response.add_event(event_deposit(
                info.sender.clone(),
                info.funds.clone(),
                amount,
            ));
            vault.rebalance(deps.storage, rcpt.supply(deps.querier)? + aum_fee + amount)?;
            response = response.add_message(rcpt.mint_msg(amount, info.sender));
        }
        ExecuteMsg::Withdraw {} => {
            let amount = must_pay(&info, rcpt.denom().as_str())?;
            let (net, burn_fee) = fee_manager.tx_fee(amount)?;
            vault.rebalance(deps.storage, rcpt.supply(deps.querier)? + aum_fee)?;
            let withdraw_funds = Vault::withdraw(deps.storage, net)?;
            response = response
                .add_event(event_withdraw(
                    info.sender.clone(),
                    withdraw_funds.clone(),
                    amount,
                ))
                .add_message(rcpt.burn_msg(amount));
            if !withdraw_funds.is_empty() {
                let send_msg = BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: withdraw_funds,
                };
                response = response.add_message(send_msg);
            }
            if burn_fee.gt(&Uint128::zero()) {
                response =
                    response.add_message(rcpt.mint_msg(burn_fee, config.fee_collector.clone()));
            }
        }
        ExecuteMsg::Callback(cb) => {
            let callback_type: CallbackType = cb.deserialize_callback()?;
            match callback_type {
                CallbackType::AfterReallocate {
                    swap_to,
                    min_return,
                } => {
                    let coin = deps
                        .querier
                        .query_balance(&env.contract.address, &config.quote_denom)?;
                    let msg = WasmMsg::Execute {
                        contract_addr: swap_to.to_string(),
                        msg: to_json_binary(&fin::ExecuteMsg::Swap(SwapRequest {
                            min_return,
                            to: None,
                            callback: None,
                        }))?,
                        funds: vec![coin],
                    };
                    let run_msg = WasmMsg::Execute {
                        contract_addr: env.contract.address.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Run {})?,
                        funds: vec![],
                    };
                    response = response.add_message(msg).add_message(run_msg);
                }
            }
        }
        ExecuteMsg::Run {} => {
            vault.rebalance(deps.storage, rcpt.supply(deps.querier)? + aum_fee)?;
        }
    }

    if aum_fee.gt(&Uint128::zero()) {
        response = response.add_message(rcpt.mint_msg(aum_fee, config.fee_collector));
    }

    FEE_MANAGER.save(deps.storage, &fee_manager)?;
    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut config = Config::load(deps.storage)?;
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    let total_supply = rcpt.supply(deps.querier)?;
    let vault = Vault::new(deps.api, &deps.querier, env.contract.address);
    match msg {
        SudoMsg::Reallocate {
            from,
            to,
            weight,
            min_return,
        } => {
            let msg =
                Vault::reallocate(deps.storage, &from, &to, weight, total_supply, min_return)?;
            let response = Response::new().add_event(event_reallocate(from, to, weight));
            Ok(response.add_message(msg))
        }
        SudoMsg::UpdateFees {
            fee_collector,
            fees,
        } => {
            config.update(deps.api, fee_collector)?;
            config.save(deps.storage)?;
            FEE_MANAGER.save(deps.storage, &FeeManager::new(fees, env.block.time)?)?;
            Ok(Response::new())
        }
        SudoMsg::RemoveAllocation { denom } => {
            Vault::remove_allocation(deps.storage, denom)?;
            Ok(Response::new())
        }
        SudoMsg::AddAllocation { denom, contract } => {
            vault.add_allocation(
                deps.storage,
                &config.quote_denom,
                &denom,
                Uint128::zero(),
                &contract,
            )?;
            vault.rebalance(deps.storage, total_supply)?;
            Ok(Response::new())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let config = Config::load(deps.storage)?;
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    match msg {
        QueryMsg::Config {} => Ok(to_json_binary(&ConfigResponse::from(config))?),
        QueryMsg::Fees {} => Ok(to_json_binary(&FEE_MANAGER.load(deps.storage)?)?),
        QueryMsg::Status {} => Ok(to_json_binary(&Vault::status(
            deps.storage,
            rcpt.supply(deps.querier)?,
        )?)?),
    }
}

#[cfg(test)]
mod tests {}
