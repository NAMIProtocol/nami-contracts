#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, ensure, ensure_eq, to_json_binary, BankMsg, Binary, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::NativeBalance;
use nami_rs::affiliate::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::{error::ContractError, events::execute_event};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Execute {
            contract_addr,
            msg,
            affiliate,
        } => {
            ensure!(!info.funds.is_empty(), ContractError::InsufficientFunds {});

            let (mut net_funds, mut fees) = if let Some(affiliate) = affiliate.as_ref() {
                let (mut net_funds, mut fees) = (Vec::new(), Vec::new());

                for fund in info.funds.iter() {
                    let fee = Decimal::from_ratio(fund.amount, Uint128::one())
                        .checked_mul(affiliate.1)?
                        .to_uint_floor();
                    let net = fund.amount.checked_sub(fee)?;
                    net_funds.push(coin(net.into(), fund.denom.clone()));
                    fees.push(coin(fee.into(), fund.denom.clone()));
                }

                (NativeBalance(net_funds), NativeBalance(fees))
            } else {
                (NativeBalance(info.funds.clone()), NativeBalance(Vec::new()))
            };
            net_funds.normalize();
            fees.normalize();

            let mut response =
                Response::new().add_event(execute_event(contract_addr.clone(), affiliate.clone()));

            if fees.0.len() > 0 {
                response = response.add_message(BankMsg::Send {
                    to_address: affiliate.unwrap().0,
                    amount: fees.into_vec(),
                });
            }

            response = response.add_message(WasmMsg::Execute {
                contract_addr,
                msg,
                funds: net_funds.into_vec(),
            });

            response = response.add_message(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::Send {
                    sender: info.sender.to_string(),
                })?,
                funds: vec![],
            });

            Ok(response)
        }
        ExecuteMsg::Send { sender } => {
            ensure_eq!(
                info.sender,
                env.contract.address,
                ContractError::Unauthorized {}
            );
            let balances = deps.querier.query_all_balances(env.contract.address)?;
            ensure!(!balances.is_empty(), ContractError::InvalidAffiliateCall {});

            Ok(Response::new().add_message(BankMsg::Send {
                to_address: sender,
                amount: balances,
            }))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> Result<Binary, ContractError> {
    Ok(to_json_binary(&())?)
}

#[cfg(test)]
mod tests {}
