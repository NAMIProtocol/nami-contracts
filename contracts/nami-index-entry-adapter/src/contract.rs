#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, ensure_eq, to_json_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::must_pay;
use nami_rs::index_entry_adapter::{
    ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg, SwapContractResponse, SwapContractsResponse,
    SwapEntry, ThenType,
};

use crate::config::Config;
use crate::error::ContractError;
use crate::index::Index;
use crate::state::Status;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const STATUS: Status = Status::new();

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::from(msg.clone());
    config.save(deps.storage)?;
    for (denom, contract) in msg.swap_contracts.iter() {
        STATUS.add_contract(deps.storage, denom, contract.clone())?;
    }
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let config = Config::load(deps.storage)?;
    match msg {
        ExecuteMsg::Deposit { index, swaps } => {
            let amount = must_pay(&info, config.base_denom.as_str())?;
            let mut msgs = Vec::new();
            let mut swap_amount = Uint128::zero();
            for swap in swaps {
                swap_amount += swap.amount;
                msgs.push(STATUS.swap_msg(
                    deps.storage,
                    SwapEntry {
                        denom: config.base_denom.clone(),
                        amount: swap.amount,
                        min_return: swap.min_return,
                    },
                    None,
                )?);
            }
            ensure_eq!(
                amount,
                swap_amount,
                ContractError::InvalidSwapData {
                    sent: amount,
                    expected: swap_amount
                }
            );

            let then = WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ThenType::Deposit {
                    sender: info.sender,
                    index,
                })?,
                funds: vec![],
            };
            Ok(Response::default().add_messages(msgs).add_message(then))
        }
        ExecuteMsg::Withdraw { index, min_return } => {
            let index = Index::load(index, &deps.querier)?;
            let amount = must_pay(&info, &index.denom)?;
            let then = WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ThenType::Swap {
                    sender: info.sender.clone(),
                    min_return,
                })?,
                funds: vec![],
            };
            Ok(Response::default()
                .add_message(index.withdraw_msg(amount)?)
                .add_message(then))
        }
        ExecuteMsg::Then(then) => {
            ensure_eq!(
                info.sender,
                env.contract.address,
                ContractError::Unauthorized {}
            );
            match then {
                ThenType::Deposit { sender, index } => {
                    let index = Index::load(index, &deps.querier)?;
                    let (deposit_msg, remaining_coins) = index.deposit_msg(&env, &deps.querier)?;
                    let mut msgs = Vec::new();
                    msgs.push(deposit_msg);
                    if !remaining_coins.is_empty() {
                        for swap in remaining_coins {
                            msgs.push(STATUS.swap_msg(
                                deps.storage,
                                swap,
                                Some(sender.to_string()),
                            )?);
                        }
                    }
                    let then = WasmMsg::Execute {
                        contract_addr: env.contract.address.to_string(),
                        msg: to_json_binary(&ThenType::Send {
                            sender,
                            min_return: None,
                        })?,
                        funds: vec![],
                    };
                    msgs.push(then.into());

                    Ok(Response::default().add_messages(msgs))
                }
                ThenType::Swap { sender, min_return } => {
                    let balances = deps.querier.query_all_balances(&env.contract.address)?;
                    let mut msgs = Vec::new();
                    for balance in balances {
                        msgs.push(STATUS.swap_msg(
                            deps.storage,
                            SwapEntry {
                                denom: balance.denom,
                                amount: balance.amount,
                                min_return: None,
                            },
                            None,
                        )?);
                    }

                    let then = WasmMsg::Execute {
                        contract_addr: env.contract.address.to_string(),
                        msg: to_json_binary(&ThenType::Send { sender, min_return })?,
                        funds: vec![],
                    };
                    msgs.push(then.into());

                    Ok(Response::default().add_messages(msgs))
                }
                ThenType::Send { sender, min_return } => {
                    let balances = deps.querier.query_all_balances(&env.contract.address)?;
                    let mut msgs = Vec::new();
                    for balance in balances {
                        if let Some(min_return) = min_return {
                            if balance.amount < min_return {
                                return Err(ContractError::InsufficientReturn {
                                    expected: min_return,
                                    returned: balance.amount,
                                });
                            }
                        }
                        msgs.push(BankMsg::Send {
                            to_address: sender.to_string(),
                            amount: coins(balance.amount.u128(), balance.denom),
                        });
                    }
                    Ok(Response::default().add_messages(msgs))
                }
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut config = Config::load(deps.storage)?;
    match msg {
        SudoMsg::AddSwapContract { denom, contract } => {
            STATUS.add_contract(deps.storage, &denom, contract)?;
            Ok(Response::default())
        }
        SudoMsg::RemoveSwapContract { denom } => {
            STATUS.remove_contract(deps.storage, &denom)?;
            Ok(Response::default())
        }
        SudoMsg::UpdateConfig { base_denom } => {
            config.base_denom = base_denom;
            config.save(deps.storage)?;
            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_json_binary(&Config::load(deps.storage)?)?),
        QueryMsg::SwapContracts {} => {
            let swap_contracts = STATUS.get_swap_contracts(deps.storage)?;
            let response = SwapContractsResponse { swap_contracts };
            Ok(to_json_binary(&response)?)
        }
        QueryMsg::SwapContract { denom } => {
            let contract = STATUS.get(deps.storage, &denom)?;
            Ok(to_json_binary(&SwapContractResponse { contract })?)
        }
    }
}

#[cfg(test)]
mod tests {}
