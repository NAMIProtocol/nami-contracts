mod asset_allocation;
pub mod config;
pub mod contract;
mod error;
mod events;
mod fee_collector;
mod vault;

pub use crate::error::ContractError;

#[cfg(test)]
pub mod testing;