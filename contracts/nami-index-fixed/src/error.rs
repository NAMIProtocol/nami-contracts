use cosmwasm_std::{CheckedFromRatioError, OverflowError, StdError};
use cw_utils::PaymentError;
use nami_rs::FeeManagerError;
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
    FeeManagerError(#[from] FeeManagerError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Missing denomination in deposit: {0}")]
    MissingDenomination(String),

    #[error("Deposit proportions must be identical")]
    DepositProportionsNotIdentical,

    #[error("Invalid: {0}")]
    Invalid(String),

    #[error("Weight must be zero to remove allocation")]
    WeightNotZero,

    #[error("Invalid weight must be greater than current weight")]
    InvalidWeight,

    #[error("Invalid denom pair")]
    InvalidDenomPair,
}
