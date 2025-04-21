use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use rujira_rs::{CallbackMsg, TokenMetadata};

use crate::FeeRates;

#[cw_serde]
pub struct InstantiateMsg {
    pub receipt: TokenMetadata,
    pub base_denom: String,
    pub fee_collector: String,
    pub fees: FeeRates,
    pub target_denoms: Vec<(String, Uint128, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {},
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum SudoMsg {
    Reallocate {
        from: String,
        to: String,
        weight: Uint128,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub enum CallbackType {
    AfterReallocate { swap_to: Addr },
}
