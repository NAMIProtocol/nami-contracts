#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdError, Uint128,
};
use cw2::set_contract_version;
use cw_storage_plus::Item;
use cw_utils::{must_pay, nonpayable};
use nami_rs::index_nav::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg};
use nami_rs::{AssetAllocation, FeeManager};
use rujira_rs::TokenFactory;

use crate::config::Config;
use crate::error::ContractError;
use crate::events::{event_deposit, event_run, event_withdraw};
use crate::vault::Vault;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

static FEE_MANAGER: Item<FeeManager> = Item::new("fee_manager");

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
    let vault = Vault::new(
        deps.api,
        &deps.querier,
        env.contract.address,
        config.quote_denom.clone(),
    );
    vault.init(deps.storage, msg.target_allocation.clone())?;
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
    let vault = Vault::new(
        deps.api,
        &deps.querier,
        env.contract.address.clone(),
        config.quote_denom.clone(),
    );
    let rcpt = TokenFactory::new(
        &env,
        format!("nami-index-{}-rcpt", env.contract.address).as_str(),
    );
    let aum_fee = fee_manager.aum_fee(env.block.time, rcpt.supply(deps.querier)?)?;
    let mut response = Response::new();
    match msg {
        ExecuteMsg::Deposit {} => {
            let amount = must_pay(&info, &config.quote_denom)?;
            let nav = vault.nav(deps.storage, Some(amount), rcpt.supply(deps.querier)?)?;
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
        ExecuteMsg::Withdraw { slippage } => {
            let amount = must_pay(&info, rcpt.denom().as_str())?;
            let (net, burn_fee) = fee_manager.tx_fee(amount)?;
            let nav = vault.nav(deps.storage, None, rcpt.supply(deps.querier)?)?;
            let withdraw_value = Decimal::from_ratio(net, Uint128::one())
                .checked_mul(nav)?
                .to_uint_floor();
            response = response
                .add_event(event_withdraw(info.sender.clone(), withdraw_value, amount))
                .add_message(rcpt.burn_msg(amount));

            if withdraw_value.gt(&Uint128::zero()) {
                response = response.add_messages(vault.withdraw(
                    deps.storage,
                    withdraw_value,
                    info.sender.clone(),
                    slippage,
                )?);
            }

            if burn_fee.gt(&Uint128::zero()) {
                response =
                    response.add_message(rcpt.mint_msg(burn_fee, config.fee_collector.clone()));
            }
        }
        ExecuteMsg::Run {} => {
            nonpayable(&info)?;
            let rebalance_msgs = vault.rebalance(deps.storage)?;
            response = response
                .add_event(event_run(info.sender.clone()))
                .add_messages(rebalance_msgs);
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
    let vault = Vault::new(
        deps.api,
        &deps.querier,
        env.contract.address,
        config.quote_denom.clone(),
    );
    match msg {
        SudoMsg::UpdateFees {
            fee_collector,
            fees,
        } => {
            config.update(deps.api, fee_collector)?;
            config.save(deps.storage)?;
            FEE_MANAGER.save(deps.storage, &FeeManager::new(fees, env.block.time)?)?;
            Ok(Response::default())
        }
        SudoMsg::AddAllocation {
            denom,
            weight,
            contract,
            oracle,
            threshold,
            slippage,
        } => {
            vault.save_allocation(
                deps.storage,
                AssetAllocation::new(denom, weight, contract, oracle, threshold, slippage),
            )?;
            Ok(Response::default())
        }
        SudoMsg::RemoveAllocation { denom } => {
            vault.remove_allocation(deps.storage, denom)?;
            Ok(Response::default())
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
    let vault = Vault::new(
        deps.api,
        &deps.querier,
        env.contract.address,
        config.quote_denom.clone(),
    );
    match msg {
        QueryMsg::Config {} => Ok(to_json_binary(&ConfigResponse::from(config))?),
        QueryMsg::Fees {} => Ok(to_json_binary(&FEE_MANAGER.load(deps.storage)?)?),
        QueryMsg::Status {} => Ok(to_json_binary(
            &vault.status(deps.storage, rcpt.supply(deps.querier)?)?,
        )?),
    }
}
