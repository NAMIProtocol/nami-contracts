use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use rujira_rs::{CallbackMsg, TokenMetadata};

use crate::{FeeManager, FeeRates};

#[cw_serde]
pub struct InstantiateMsg {
    pub receipt: TokenMetadata,
    pub quote_denom: String,
    pub fee_collector: String,
    pub fees: FeeRates,
    pub target_allocations: Vec<(String, Uint128, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {},
    Callback(CallbackMsg),
    Run {},
}

#[cw_serde]
pub enum SudoMsg {
    Reallocate {
        from: String,
        to: String,
        weight: Uint128,
        min_return: Option<Uint128>,
    },
    UpdateFees {
        fee_collector: Option<String>,
        fees: FeeRates,
    },
    RemoveAllocation {
        denom: String,
    },
    AddAllocation {
        denom: String,
        contract: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(VaultStatusResponse)]
    Status {},

    #[returns(FeeManager)]
    Fees {},

    #[returns(ConfigResponse)]
    Config {},
}

#[cw_serde]
pub enum CallbackType {
    AfterReallocate { swap_to: Addr, min_return: Option<Uint128> },
}

#[cw_serde]
pub struct VaultStatusResponse {
    pub total_shares: Uint128,
    pub allocation: Vec<(String, Uint128)>,
}

#[cw_serde]
pub struct ConfigResponse {
    pub quote_denom: String,
    pub fee_collector: String,
}
