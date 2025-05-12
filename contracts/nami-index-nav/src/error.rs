use cosmwasm_std::{CheckedFromRatioError, OverflowError, StdError};
use cw_utils::PaymentError;
use nami_rs::{AssetAllocationError, FeeManagerError};
use rujira_rs::OracleError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    CheckedFromRatio(#[from] CheckedFromRatioError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    OracleError(#[from] OracleError),

    #[error("{0}")]
    FeeManagerError(#[from] FeeManagerError),

    #[error("{0}")]
    AssetAllocationError(#[from] AssetAllocationError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("SlippageExceeded")]
    SlippageExceeded {},

    #[error("Weight must be zero to remove allocation")]
    WeightNotZero,

    #[error("Missing or duplicate quote allocation")]
    MissingQuoteAllocation,

    #[error("Weight must sum to 1")]
    WeightOne,

    #[error("Invalid quote denom")]
    InvalidQuoteDenom,

    #[error("Swap Contract needed for non-quote allocation")]
    InvalidSwapContract,

    #[error("Slippage must be greater than 0 and less than 1")]
    SlippageOne,

    #[error("Invalid: {0}")]
    Invalid(String),
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
