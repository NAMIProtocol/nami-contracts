use cosmwasm_std::{CheckedFromRatioError, OverflowError, StdError};
use cw_utils::PaymentError;
use nami_rs::FeeManagerError;
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

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Invalid: {0}")]
    Invalid(String),
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
