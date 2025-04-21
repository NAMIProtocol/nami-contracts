use cosmwasm_schema::cw_serde;
use cosmwasm_std::{QuerierWrapper, Decimal};
use rujira_rs::{Layer1Asset, Oracle, OracleError};

#[cw_serde]
pub enum OracleConfig {
    Layer1(Layer1Asset),
}

impl Oracle for OracleConfig {
    fn price(&self, q: QuerierWrapper) -> Result<Decimal, OracleError> {
        match self {
            OracleConfig::Layer1(asset) => asset.price(q)
        }
    }
}