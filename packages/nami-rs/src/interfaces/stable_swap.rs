use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use rujira_rs::CallbackData;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    MultiSwap { to: Addr, denom: String },
    Swap(SwapRequest),
}

#[cw_serde]
pub enum SudoMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub struct SwapRequest {
    pub denom: Option<String>, // option is only for fin compatibility must be passed if want to use stableswap
    pub min_return: Option<Uint128>,
    pub to: Option<String>,
    pub callback: Option<CallbackData>,
}
