#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, ensure, ensure_eq, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Empty, Env,
    MessageInfo, Order, Response, StdError, WasmMsg,
};
use cw2::set_contract_version;
use nami_rs::affiliate::{ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg};

use crate::{config::Config, error::ContractError, events::execute_event};
use cosmwasm_std::Addr;
use cw_storage_plus::Map;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

static WHITELIST: Map<Addr, Empty> = Map::new("whitelist");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config::from(msg.clone());
    config.validate()?;
    config.save(deps.storage)?;
    if let Some(whitelist) = msg.whitelist {
        for addr in whitelist {
            WHITELIST.save(
                deps.storage,
                deps.api.addr_validate(addr.as_str())?,
                &Empty {},
            )?;
        }
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
        ExecuteMsg::Execute {
            contract_addr,
            msg,
            affiliate,
        } => {
            ensure!(!info.funds.is_empty(), ContractError::InsufficientFunds {});
            ensure!(
                WHITELIST.has(
                    deps.storage,
                    deps.api.addr_validate(contract_addr.as_str())?
                ),
                ContractError::Unauthorized {}
            );
            let mut net_funds = info.funds.clone();
            let mut response =
                Response::new().add_event(execute_event(contract_addr.clone(), affiliate.clone()));

            if let Some((addr, bps)) = affiliate {
                ensure!(
                    bps <= config.max_affiliate_fee_bps,
                    ContractError::InvalidAffiliateFee {
                        max: config.max_affiliate_fee_bps
                    }
                );
                let pairs: Vec<(Coin, Coin)> = info
                    .funds
                    .iter()
                    .map(|c| -> Result<(Coin, Coin), ContractError> {
                        let fee = c
                            .amount
                            .checked_mul(bps.into())?
                            .checked_div(10_000u16.into())?;
                        let net = c.amount.checked_sub(fee)?;
                        Ok((
                            coin(net.u128(), c.denom.clone()),
                            coin(fee.u128(), c.denom.clone()),
                        ))
                    })
                    .collect::<Result<_, _>>()?;

                let (nets, mut fees): (Vec<Coin>, Vec<Coin>) = pairs.into_iter().unzip();
                fees.retain(|c| !c.amount.is_zero());

                if !fees.is_empty() {
                    response = response.add_message(BankMsg::Send {
                        to_address: addr.clone(),
                        amount: fees,
                    });
                }

                net_funds = nets;
            }

            response = response
                .add_message(WasmMsg::Execute {
                    contract_addr,
                    msg,
                    funds: net_funds.clone(),
                })
                .add_message(WasmMsg::Execute {
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
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::AddWhitelisted { addr } => {
            WHITELIST.save(
                deps.storage,
                deps.api.addr_validate(addr.as_str())?,
                &Empty {},
            )?;
            Ok(Response::default())
        }
        SudoMsg::RemoveWhitelisted { addr } => {
            WHITELIST.remove(deps.storage, deps.api.addr_validate(addr.as_str())?);
            Ok(Response::default())
        }
        SudoMsg::UpdateConfig {
            max_affiliate_fee_bps,
        } => {
            let mut config = Config::load(deps.storage)?;
            config.update(max_affiliate_fee_bps)?;
            config.save(deps.storage)?;
            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::Whitelists {} => {
            let keys = WHITELIST
                .keys(deps.storage, None, None, Order::Ascending)
                .collect::<Result<Vec<Addr>, StdError>>()?;
            Ok(to_json_binary(&keys)?)
        }
    }
}
