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
    UpdateAllocation(Vec<AssetAllocation<OracleConfig>>),
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
pub struct MigrateMsg {}

#[cw_serde]
pub struct ConfigResponse {
    pub quote_denom: String,
    pub fee_collector: String,
}

#[cw_serde]
pub struct VaultStatusResponse {
    // Redemption rate is the amount of quote denom per share
    pub redemption_rate: Decimal,
    // Total number of shares
    pub shares: Uint128,
    // Total Net Asset Value of the vault
    pub nav: Uint128,
    // NAV per share
    pub nav_per_share: Decimal,
    // Allocation of each asset
    pub allocation: Vec<AllocationResponse>,
}

#[cw_serde]
pub struct AllocationResponse {
    // Denom of the asset
    pub denom: String,
    // Swap contract of the asset
    pub swap_contract: Option<String>,
    // Balance of the asset
    pub balance: Uint128,
    // Price of the asset
    pub price: Decimal,
    // Weight of the asset
    pub weight: Decimal,
    // Threshold is the minimum price change required to trigger a rebalance
    pub threshold: Decimal,
    // Slippage is the maximum price change allowed before a rebalance is triggered
    pub slippage: Decimal,
}
