use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use rujira_rs::TokenMetadata;

#[cw_serde]
pub struct InstantiateMsg {
    pub fee: Decimal,
    pub streaming_denom: String,
    pub fee_contract: Addr,
    pub swap_router: Addr,
    pub operator: Addr,
    pub receipt: TokenMetadata,
    pub arber_config: ArberConfig,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Actions relating to Stream-based yield
    Stream(StreamMsg),
    /// Actions relating to Compound deposits
    Compound(CompoundMsg),
    /// Actions relating to arbitrage for yield optimization
    Arbitrage(ArbMsg),
}

#[cw_serde]
pub enum StreamMsg {
    Deposit {},
    Claim {},
    Withdraw {
        amount: Option<Uint128>,
        denom: String,
    },
}

#[cw_serde]
pub enum CompoundMsg {
    Deposit {},
    Withdraw { denom: String },
}

#[cw_serde]
pub enum ArbMsg {
    Swap { to: String },
    Rebalance { vaults: Vec<Rebalance> },
}

#[cw_serde]
pub enum SudoMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub struct AccountingStatusResponse {
    pub streaming_deposits: Uint128,
    pub streaming_revenue: Uint128,
    pub compound_shares: Uint128,
    pub compound_size: Uint128,
    pub compound_ratio: Decimal,
}

#[cw_serde]
pub struct AccountingUserResponse {
    pub addr: String,
    pub deposited: Uint128,
    pub pending_revenue: Uint128,
}

#[cw_serde]
pub struct ArberConfig {
    pub premium: Decimal,
}

#[cw_serde]
pub struct Rebalance {
    pub address: Addr,
    pub weight: Decimal,
}
