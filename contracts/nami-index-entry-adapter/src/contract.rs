#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, ensure_eq, to_json_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, Uint128, WasmMsg,
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
        STATUS.add_contract(
            deps.storage,
            &deps.querier,
            denom,
            contract.clone(),
            config.quote_denom.clone(),
        )?;
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
            let amount = must_pay(&info, config.quote_denom.as_str())?;
            let (swap_amount, msgs) = swaps.into_iter().try_fold(
                (Uint128::zero(), Vec::<CosmosMsg>::new()),
                |(acc, mut msgs),
                 SwapEntry {
                     denom,
                     amount,
                     min_return,
                 }|
                 -> Result<(Uint128, Vec<CosmosMsg>), ContractError> {
                    let msg = STATUS.swap_msg(
                        deps.storage,
                        &denom,
                        SwapEntry {
                            denom: config.quote_denom.clone(),
                            amount,
                            min_return,
                        },
                        None,
                    )?;
                    msgs.push(msg);
                    Ok((acc + amount, msgs))
                },
            )?;
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
                msg: to_json_binary(&ExecuteMsg::Then(ThenType::Deposit {
                    sender: info.sender,
                    index,
                }))?,
                funds: vec![],
            };
            Ok(Response::default().add_messages(msgs).add_message(then))
        }
        ExecuteMsg::Withdraw { index, min_return } => {
            let index = Index::load(index, &deps.querier)?;
            let amount = must_pay(&info, &index.denom)?;
            let then = WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::Then(ThenType::Swap {
                    sender: info.sender.clone(),
                    min_return,
                }))?,
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
                                swap.denom.clone().as_str(),
                                swap,
                                Some(sender.to_string()),
                            )?);
                        }
                    }
                    let then = WasmMsg::Execute {
                        contract_addr: env.contract.address.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Then(ThenType::Send {
                            sender,
                            min_return: None,
                        }))?,
                        funds: vec![],
                    };
                    msgs.push(then.into());

                    Ok(Response::default().add_messages(msgs))
                }
                ThenType::Swap { sender, min_return } => {
                    let balances = deps.querier.query_all_balances(&env.contract.address)?;
                    let mut msgs: Vec<CosmosMsg> = balances
                        .into_iter()
                        .map(|balance| {
                            STATUS
                                .swap_msg(
                                    deps.storage,
                                    balance.denom.as_str(),
                                    SwapEntry {
                                        denom: balance.denom.clone(),
                                        amount: balance.amount,
                                        min_return: None,
                                    },
                                    None,
                                )
                                .map(Into::into)
                        })
                        .collect::<Result<_, ContractError>>()?;

                    msgs.push(
                        WasmMsg::Execute {
                            contract_addr: env.contract.address.to_string(),
                            msg: to_json_binary(&ExecuteMsg::Then(ThenType::Send {
                                sender,
                                min_return,
                            }))?,
                            funds: vec![],
                        }
                        .into(),
                    );

                    Ok(Response::default().add_messages(msgs))
                }
                ThenType::Send { sender, min_return } => {
                    let balances = deps.querier.query_all_balances(&env.contract.address)?;
                    let msgs: Vec<CosmosMsg> = balances
                        .into_iter()
                        .map(|balance| {
                            if let Some(min) = min_return {
                                if balance.amount < min {
                                    return Err(ContractError::InsufficientReturn {
                                        expected: min,
                                        returned: balance.amount,
                                    });
                                }
                            }
                            Ok(BankMsg::Send {
                                to_address: sender.to_string(),
                                amount: coins(balance.amount.u128(), balance.denom),
                            }
                            .into())
                        })
                        .collect::<Result<_, ContractError>>()?;
                    Ok(Response::default().add_messages(msgs))
                }
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let config = Config::load(deps.storage)?;
    match msg {
        SudoMsg::AddSwapContract { denom, contract } => {
            STATUS.add_contract(
                deps.storage,
                &deps.querier,
                &denom,
                contract.clone(),
                config.quote_denom.clone(),
            )?;
            Ok(Response::default())
        }
        SudoMsg::RemoveSwapContract { denom } => {
            STATUS.remove_contract(deps.storage, &denom)?;
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
            Ok(to_json_binary(&SwapContractsResponse { swap_contracts })?)
        }
        QueryMsg::SwapContract { denom } => {
            let contract = STATUS.get(deps.storage, &denom)?;
            Ok(to_json_binary(&SwapContractResponse { contract })?)
        }
    }
}

#[cfg(test)]
mod tests {}
