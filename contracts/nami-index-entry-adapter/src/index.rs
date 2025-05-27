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
        let status: index_fixed::VaultStatusResponse =
            querier.query_wasm_smart(&address, &index_fixed::QueryMsg::Status {})?;
        let denom = format!("x/nami-index-fixed-{}-rcpt", address);
        Ok(Self {
            address,
            allocation: status.allocation,
            denom,
        })
    }

    pub fn deposit_msg(
        &self,
        env: &Env,
        querier: &QuerierWrapper,
    ) -> Result<(CosmosMsg, Vec<SwapEntry>), ContractError> {
        let infos: Vec<(String, Uint128, Uint128, Uint128)> = self
            .allocation
            .iter()
            .map(|(denom, weight)| {
                let coin = querier.query_balance(env.contract.address.clone(), denom)?;
                if coin.amount.is_zero() {
                    return Err(ContractError::IndexError(
                        IndexErrors::InvalidDepositAmount(denom.clone()),
                    ));
                }
                let units = Decimal::from_ratio(coin.amount, *weight).to_uint_floor();
                Ok((denom.clone(), *weight, coin.amount, units))
            })
            .collect::<Result<_, ContractError>>()?;

        let share = infos
            .iter()
            .map(|(_, _, _, u)| *u)
            .min()
            .filter(|u| !u.is_zero())
            .ok_or_else(|| ContractError::IndexError(IndexErrors::InvalidDepositProportions))?;

        let (funds, remaining_coins) = infos.into_iter().try_fold(
            (Vec::with_capacity(self.allocation.len()), Vec::new()),
            |(mut funds, mut rems),
             (denom, weight, total, _)|
             -> Result<(Vec<Coin>, Vec<SwapEntry>), ContractError> {
                let used = share.checked_mul(weight)?;
                let rem = total.checked_sub(used)?;
                funds.push(Coin::new(used, denom.clone()));
                if !rem.is_zero() {
                    rems.push(SwapEntry {
                        denom,
                        amount: rem,
                        min_return: None,
                    });
                }
                Ok((funds, rems))
            },
        )?;

        let msg = WasmMsg::Execute {
            contract_addr: self.address.clone(),
            msg: to_json_binary(&index_fixed::ExecuteMsg::Deposit {})?,
            funds,
        }
        .into();

        Ok((msg, remaining_coins))
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
        Coin, CosmosMsg, Empty, QuerierWrapper, Uint128, WasmMsg,
    };

    fn make_index() -> Index {
        Index {
            address: "index_address".to_string(),
            allocation: vec![
                ("usdc".to_string(), Uint128::new(1000)),
                ("eth".to_string(), Uint128::new(500)),
            ],
            denom: "nami-index-index_address-rcpt".to_string(),
        }
    }

    #[test]
    fn test_index_deposit_msg_no_rest() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            &[Coin::new(1000u128, "usdc"), Coin::new(500u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = make_index();

        let (deposit_msg, remaining_coins) = index.deposit_msg(&env, &querier).unwrap();

        assert!(remaining_coins.is_empty());
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            funds,
            ..
        }) = deposit_msg
        {
            assert_eq!(contract_addr, "index_address");
            assert_eq!(funds.len(), 2);
            assert_eq!(funds[0].amount, Uint128::new(1000));
            assert_eq!(funds[1].amount, Uint128::new(500));
        } else {
            panic!("Expected WasmMsg::Execute");
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
        let index = make_index();

        let (deposit_msg, remaining_coins) = index.deposit_msg(&env, &querier).unwrap();

        assert_eq!(remaining_coins.len(), 2);
        assert_eq!(remaining_coins[0].denom, "usdc");
        assert_eq!(remaining_coins[0].amount, Uint128::new(200));
        assert_eq!(remaining_coins[1].denom, "eth");
        assert_eq!(remaining_coins[1].amount, Uint128::new(50));

        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            funds,
            ..
        }) = deposit_msg
        {
            assert_eq!(contract_addr, "index_address");
            assert_eq!(funds.len(), 2);
            assert_eq!(funds[0].amount, Uint128::new(1000));
            assert_eq!(funds[1].amount, Uint128::new(500));
        } else {
            panic!("Expected WasmMsg::Execute");
        }
    }

    #[test]
    fn test_index_deposit_msg_zero_balance_error() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            &[Coin::new(0u128, "usdc"), Coin::new(500u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = make_index();

        let err = index.deposit_msg(&env, &querier).unwrap_err();
        match err {
            ContractError::IndexError(IndexErrors::InvalidDepositAmount(denom)) => {
                assert_eq!(denom, "usdc");
            }
            _ => panic!("Expected InvalidDepositAmount error"),
        }
    }

    #[test]
    fn test_index_deposit_msg_invalid_proportions_error() {
        let env = mock_env();
        let mock_querier: MockQuerier<Empty> = MockQuerier::new(&[(
            &env.contract.address.to_string(),
            // Both coins too small relative to weights → share = 0
            &[Coin::new(500u128, "usdc"), Coin::new(250u128, "eth")],
        )]);
        let querier = QuerierWrapper::new(&mock_querier);
        let index = make_index();

        let err = index.deposit_msg(&env, &querier).unwrap_err();
        match err {
            ContractError::IndexError(IndexErrors::InvalidDepositProportions) => {}
            _ => panic!("Expected InvalidDepositProportions error"),
        }
    }
}
