use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coins, to_json_binary, Coin, CosmosMsg, Decimal, Env, QuerierWrapper, StdResult, Uint128,
    WasmMsg,
};
use nami_rs::{index_entry_adapter::SwapEntry, index_fixed};
use thiserror::Error;

use crate::ContractError;

#[cw_serde]
pub struct Index {
    pub address: String,
    pub allocation: Vec<(String, Uint128)>,
    pub denom: String,
}

impl Index {
    pub fn load(address: String, querier: &QuerierWrapper) -> StdResult<Self> {
        let status: index_fixed::StatusResponse =
            querier.query_wasm_smart(address.clone(), &index_fixed::QueryMsg::Status {})?;
        Ok(Self {
            address: address.clone(),
            allocation: status.allocation,
            denom: format!("nami-index-{}-rcpt", address),
        })
    }

    pub fn deposit_msg(
        &self,
        env: &Env,
        querier: &QuerierWrapper,
    ) -> Result<(CosmosMsg, Vec<SwapEntry>), ContractError> {
        let mut shares = None;
        let mut coins = Vec::new();
        let mut remaining_coins = Vec::new();
        for (denom, weight) in &self.allocation {
            let coin = querier.query_balance(env.contract.address.clone(), denom)?;

            if coin.amount.is_zero() {
                return Err(ContractError::IndexError(
                    IndexErrors::InvalidDepositAmount(denom.clone()),
                ));
            }

            let units = Decimal::from_ratio(coin.amount, *weight).to_uint_floor();

            match shares {
                Some(ref s) if s != &units => {
                    return Err(ContractError::IndexError(
                        IndexErrors::InvalidDepositProportions,
                    ));
                }
                None => shares = Some(units),
                _ => {}
            }
            let amount = units.checked_mul(*weight)?;
            let remaining = coin.amount.checked_sub(amount)?;
            if !remaining.is_zero() {
                remaining_coins.push(SwapEntry {
                    denom: denom.clone(),
                    amount: remaining,
                    min_return: None,
                });
            }
            coins.push(Coin::new(amount, denom.clone()));
        }

        if coins.is_empty() {
            return Err(ContractError::IndexError(
                IndexErrors::InvalidDepositProportions,
            ));
        }

        Ok((
            WasmMsg::Execute {
                contract_addr: self.address.clone(),
                msg: to_json_binary(&index_fixed::ExecuteMsg::Deposit {})?,
                funds: coins,
            }
            .into(),
            remaining_coins,
        ))
    }

    pub fn withdraw_msg(&self, amount: Uint128) -> Result<CosmosMsg, ContractError> {
        Ok(WasmMsg::Execute {
            contract_addr: self.address.clone(),
            msg: to_json_binary(&index_fixed::ExecuteMsg::Withdraw {})?,
            funds: coins(amount.into(), self.denom.clone()),
        }
        .into())
    }
}

#[derive(Error, Debug)]
pub enum IndexErrors {
    #[error("Invalid deposit amount for {0}")]
    InvalidDepositAmount(String),

    #[error("Deposit proportions do not match allocation")]
    InvalidDepositProportions,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        testing::{mock_env, MockQuerier},
        Empty, QuerierWrapper, Uint128,
    };

    #[test]
    fn test_index_deposit_msg_no_rest() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            &[Coin::new(1000u128, "usdc"), Coin::new(500u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = Index {
            address: "index_address".to_string(),
            allocation: vec![
                ("usdc".to_string(), Uint128::new(1000)),
                ("eth".to_string(), Uint128::new(500)),
            ],
            denom: "nami-index-index_address-rcpt".to_string(),
        };

        let (deposit_msg, remaining_coins) = index.deposit_msg(&env, &querier).unwrap();

        assert_eq!(remaining_coins.len(), 0);
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: _,
            funds,
        }) = deposit_msg
        {
            assert_eq!(contract_addr, "index_address");
            assert_eq!(funds.len(), 2);
        } else {
            panic!("Expected WasmMsg::Execute message");
        }
    }

    #[test]
    fn test_index_deposit_msg_with_rest() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            &[Coin::new(1200u128, "usdc"), Coin::new(550u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = Index {
            address: "index_address".to_string(),
            allocation: vec![
                ("usdc".to_string(), Uint128::new(1000)),
                ("eth".to_string(), Uint128::new(500)),
            ],
            denom: "nami-index-index_address-rcpt".to_string(),
        };

        let (deposit_msg, remaining_coins) = index.deposit_msg(&env, &querier).unwrap();

        assert_eq!(remaining_coins.len(), 2);
        assert_eq!(remaining_coins[0].denom, "usdc");
        assert_eq!(remaining_coins[0].amount, Uint128::new(200));
        assert_eq!(remaining_coins[1].denom, "eth");
        assert_eq!(remaining_coins[1].amount, Uint128::new(50));
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: _,
            funds,
        }) = deposit_msg
        {
            assert_eq!(contract_addr, "index_address");
            assert_eq!(funds.len(), 2);
            assert_eq!(funds[0].denom, "usdc");
            assert_eq!(funds[1].denom, "eth");
            assert_eq!(funds[0].amount, Uint128::new(1000));
            assert_eq!(funds[1].amount, Uint128::new(500));
        } else {
            panic!("Expected WasmMsg::Execute message");
        }
    }

    #[test]
    fn test_index_deposit_msg_invalid_proportion() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            &[Coin::new(1200u128, "usdc"), Coin::new(1000u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = Index {
            address: "index_address".to_string(),
            allocation: vec![
                ("usdc".to_string(), Uint128::new(1000)),
                ("eth".to_string(), Uint128::new(500)),
            ],
            denom: "nami-index-index_address-rcpt".to_string(),
        };

        let err = index.deposit_msg(&env, &querier).unwrap_err();
        match err {
            ContractError::IndexError(IndexErrors::InvalidDepositProportions) => {}
            _ => panic!("Expected IndexErrors::InvalidDepositProportions"),
        }
    }
}
