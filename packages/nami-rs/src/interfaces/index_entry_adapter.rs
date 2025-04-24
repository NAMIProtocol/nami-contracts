use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub base_denom: String,
    pub swap_contracts: Vec<(String, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {
        index: String,
        swaps: Vec<SwapEntry>,
    },
    Withdraw {
        index: String,
        min_return: Option<Uint128>,
    },
    Then(ThenType),
}

#[cw_serde]
pub enum SudoMsg {
    AddSwapContract { denom: String, contract: String },
    RemoveSwapContract { denom: String },
    UpdateConfig { base_denom: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(SwapContractsResponse)]
    SwapContracts {},

    #[returns(SwapContractResponse)]
    SwapContract { denom: String },
}

#[cw_serde]
pub enum ThenType {
    Deposit {
        sender: Addr,
        index: String,
    },
    Swap {
        sender: Addr,
        min_return: Option<Uint128>,
    },
    Send {
        sender: Addr,
        min_return: Option<Uint128>,
    },
}

#[cw_serde]
pub struct SwapEntry {
    pub denom: String,
    pub amount: Uint128,
    pub min_return: Option<Uint128>,
}

#[cw_serde]
pub struct ConfigResponse {
    pub base_denom: String,
}

#[cw_serde]
pub struct SwapContractsResponse {
    pub swap_contracts: Vec<(String, String)>,
}

#[cw_serde]
pub struct SwapContractResponse {
    pub contract: String,
}
