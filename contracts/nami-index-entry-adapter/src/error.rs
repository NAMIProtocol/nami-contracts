use cosmwasm_std::{CheckedFromRatioError, OverflowError, StdError, Uint128};
use cw_utils::PaymentError;
use thiserror::Error;

use crate::index::IndexErrors;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    IndexError(#[from] IndexErrors),

    #[error("{0}")]
    CheckedFromRatio(#[from] CheckedFromRatioError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Insufficient Return expected {expected} got {returned}")]
    InsufficientReturn {
        expected: Uint128,
        returned: Uint128,
    },

    #[error("Invalid swap data: sent {sent} expected {expected}")]
    InvalidSwapData { sent: Uint128, expected: Uint128 },

    #[error("Invalid: {0}")]
    Invalid(String),

    #[error("Swap contract not found: {0}")]
    SwapContractNotFound(String),

    #[error("Invalid quote denom")]
    InvalidQuoteDenom,
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
