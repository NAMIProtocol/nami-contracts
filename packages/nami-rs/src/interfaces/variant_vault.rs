use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Timestamp, Uint128};

use rujira_rs::TokenMetadata;

#[cw_serde]
pub struct InstantiateMsg {
    pub base_denom: String,
    pub yield_denom: String,
    pub yield_contract: String,
    pub adapter: String,
    pub receipt: TokenMetadata,
    pub operator: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {},
    ManageFunds { action: Actions, amount: Uint128 },
}

#[cw_serde]
pub enum SudoMsg {
    AddWhitelisted { addr: String },
    RemoveWhitelisted { addr: String },
    UpdateConfig { config: UpdateConfig },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(StatusResponse)]
    Status {},

    #[returns(Decimal)]
    Ratio {},

    #[returns(ConfigResponse)]
    Config {},

    #[returns(WhitelistedResponse)]
    Whitelisted {
        limit: Option<u8>,
        start_after: Option<String>,
    },
}

#[cw_serde]
pub enum Actions {
    Deposit,
    Withdraw,
}

#[cw_serde]
pub struct StatusResponse {
    pub last_updated: Timestamp,
    pub deposit_pool: PoolResponse,
}

#[cw_serde]
pub struct UpdateConfig {
    pub adapter: Option<String>,
    pub receipt: Option<TokenMetadata>,
    pub operator: Option<String>,
}

#[cw_serde]
pub struct ConfigResponse {
    pub base_denom: String,
    pub yield_denom: String,
    pub yield_contract: String,
    pub adapter: String,
    pub receipt: TokenMetadata,
    pub operator: String,
}

#[cw_serde]
pub struct PoolResponse {
    /// The total deposits into the pool
    pub size: Uint128,
    /// The total ownership of the pool
    pub shares: Uint128,
    /// Ratio of shares / size
    pub ratio: Decimal,
}

#[cw_serde]
pub struct WhitelistedResponse {
    pub whitelisted: Vec<String>,
}
