use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Decimal;
use rujira_rs::TokenMetadata;

use crate::{FeeRates, OracleConfig};

#[cw_serde]
pub struct InstantiateMsg {
    pub receipt: TokenMetadata,
    pub base_denom: String,
    pub fee_collector: String,
    pub fees: FeeRates,
    pub target_denoms: Vec<(String, Decimal, String, OracleConfig)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {},
    Run {},
}

#[cw_serde]
pub enum SudoMsg {
    // all the configuration must be in the sudo message, like set fees change index composition etc
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
