use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Uint128};
use rujira_rs::TokenMetadata;

use crate::{AssetAllocation, FeeManager, FeeRates, OracleConfig};

#[cw_serde]
pub struct InstantiateMsg {
    pub receipt: TokenMetadata,
    pub quote_denom: String,
    pub fee_collector: String,
    pub fees: FeeRates,
    pub target_allocation: Vec<AssetAllocation<OracleConfig>>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw { slippage: Option<Decimal> },
    Run {},
}

#[cw_serde]
pub enum SudoMsg {
    UpdateFees {
        fee_collector: Option<String>,
        fees: FeeRates,
    },
    UpdateAllocation {
        target_allocation: Vec<AssetAllocation<OracleConfig>>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(FeeManager)]
    Fees {},
    #[returns(VaultStatusResponse)]
    Status {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub quote_denom: String,
    pub fee_collector: String,
}

#[cw_serde]
pub struct VaultStatusResponse {
    pub nav: Decimal,
    pub shares: Uint128,
    pub total_value: Uint128,
    pub allocation: Vec<(String, Uint128, Decimal, Decimal)>,
}
