use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, OverflowError, Timestamp, Uint128};
use thiserror::Error;

/// Configuration of optional fee rates.
///
/// - `management`: annualized fee rate (e.g., 0.02 for 2% per year)
/// - `performance`: performance fee rate applied to gains above the high‐water mark
/// - `transaction`: fee rate applied to each transaction
#[cw_serde]
pub struct FeeRates {
    pub management: Option<Decimal>,
    pub performance: Option<Decimal>,
    pub transaction: Option<Decimal>,
}

/// Fee manager
///
/// - `last_accrual_time`: last time fees were accrued in the management fee
/// - `high_water_mark`: highest value in order to charge performance fee
/// - `rates`: fee rates
#[cw_serde]
pub struct FeeManager {
    pub last_accrual_time: Timestamp,
    pub high_water_mark: Uint128,
    pub rates: FeeRates,
}

impl FeeManager {
    /// Creates a new `FeeManager` with the given fee rates, initializing
    /// the high‐water mark to zero and setting the last accrual time to `now`.
    pub fn new(rates: FeeRates, now: Timestamp) -> Result<Self, FeeManagerError> {
        Ok(FeeManager {
            last_accrual_time: now,
            high_water_mark: Uint128::zero(),
            rates,
        })
    }

    /// Calculate and accrue the management (AUM) fee since the last accrual.
    ///
    /// The management fee is an annual rate; this prorates it based on
    /// the elapsed time (`now - last_accrual_time`) over a 365.25‑day year.
    ///
    /// On success, updates `last_accrual_time` to `now` and returns
    /// the fee amount to be deducted from `total_supply`.
    pub fn aum_fee(
        &mut self,
        now: Timestamp,
        total_supply: Uint128,
    ) -> Result<Uint128, FeeManagerError> {
        let rate = self
            .rates
            .management
            .ok_or(FeeManagerError::RateNotConfigured {})?;

        let elapsed = now
            .seconds()
            .saturating_sub(self.last_accrual_time.seconds());

        if elapsed == 0 {
            return Ok(Uint128::zero());
        }

        const SECS_PER_YEAR: u64 = 31_557_600;
        let year_fraction = Decimal::from_ratio(elapsed as u128, SECS_PER_YEAR as u128);
        let effective = rate.checked_mul(year_fraction)?;

        let fee_amount = Decimal::from_ratio(total_supply, Uint128::one())
            .checked_mul(effective)?
            .to_uint_floor();

        self.last_accrual_time = now;
        Ok(fee_amount)
    }

    /// Calculate and deduct the transaction fee from `amount`.
    ///
    /// The transaction fee is a flat rate; this prorates it based on
    /// the amount (`amount`) over a 1‑day year.
    ///
    /// On success, returns the net amount after deducting the fee and the fee amount.
    pub fn tx_fee(&self, amount: Uint128) -> Result<(Uint128, Uint128), FeeManagerError> {
        let rate = self
            .rates
            .transaction
            .ok_or(FeeManagerError::RateNotConfigured {})?;

        let fee = Decimal::from_ratio(amount, Uint128::one())
            .checked_mul(rate)?
            .to_uint_floor();
        let net = amount.checked_sub(fee)?;
        Ok((net, fee))
    }

    /// Calculate and deduct the performance fee from `current_value`.
    ///
    /// The performance fee is a flat rate; this prorates it based on
    /// the gain (`current_value - high_water_mark`).
    ///
    /// On success, returns the fee amount.
    pub fn perf_fee(&mut self, current_value: Uint128) -> Result<Uint128, FeeManagerError> {
        let rate = self
            .rates
            .performance
            .ok_or(FeeManagerError::RateNotConfigured {})?;

        if current_value > self.high_water_mark {
            let gain = current_value.checked_sub(self.high_water_mark)?;
            let fee = Decimal::from_ratio(gain, Uint128::one())
                .checked_mul(rate)?
                .to_uint_floor();
            self.high_water_mark = current_value;
            Ok(fee)
        } else {
            Ok(Uint128::zero())
        }
    }
}

#[derive(Error, Debug)]
pub enum FeeManagerError {
    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("RateNotConfigured")]
    RateNotConfigured {},
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{Decimal, Timestamp, Uint128};

    fn make_rates(mng: u64, perf: u64, tx: u64) -> FeeRates {
        FeeRates {
            management: Some(Decimal::percent(mng)),
            performance: Some(Decimal::percent(perf)),
            transaction: Some(Decimal::percent(tx)),
        }
    }

    #[test]
    fn new_initializes_state() {
        let now = Timestamp::from_seconds(1_000);
        let rates = make_rates(1, 20, 5);
        let mgr = FeeManager::new(rates.clone(), now).unwrap();
        assert_eq!(mgr.last_accrual_time, now);
        assert_eq!(mgr.high_water_mark, Uint128::zero());
        assert_eq!(mgr.rates.management, rates.management);
    }

    #[test]
    fn aum_fee_zero_elapsed() {
        let now = Timestamp::from_seconds(5_000);
        let mut mgr = FeeManager::new(make_rates(10, 20, 1), now).unwrap();
        let res = mgr.aum_fee(now, Uint128::from(1_000u128)).unwrap();
        assert_eq!(res, Uint128::zero());
    }

    #[test]
    fn aum_fee_one_year() {
        let start = Timestamp::from_seconds(0);
        let mut mgr = FeeManager::new(make_rates(10, 20, 1), start).unwrap();
        // advance exactly one year
        let elapsed = 31_557_600u64;
        let later = Timestamp::from_seconds(elapsed);
        let total = Uint128::from(1_000_000u128);

        let fee = mgr.aum_fee(later, total).unwrap();
        // fee = total * 0.10 annual
        let expected = Decimal::from_ratio(total, Uint128::one())
            .checked_mul(Decimal::percent(10))
            .unwrap()
            .to_uint_floor();
        assert_eq!(fee, expected);
        assert_eq!(mgr.last_accrual_time, later);
    }

    #[test]
    fn aum_fee_half_year() {
        let start = Timestamp::from_seconds(0);
        let mut mgr = FeeManager::new(make_rates(10, 20, 1), start).unwrap();
        let elapsed = 31_557_600u64 / 2;
        let later = Timestamp::from_seconds(elapsed);
        let total = Uint128::from(2_000_000u128);

        let fee = mgr.aum_fee(later, total).unwrap();
        // fee = total * 0.10 * 0.5
        let half_rate = Decimal::percent(10)
            .checked_mul(Decimal::from_ratio(elapsed as u128, 31_557_600u128))
            .unwrap();
        let expected = Decimal::from_ratio(total, Uint128::one())
            .checked_mul(half_rate)
            .unwrap()
            .to_uint_floor();
        assert_eq!(fee, expected);
    }

    #[test]
    fn tx_fee_normal_and_zero() {
        let mgr = FeeManager::new(make_rates(1, 2, 5), Timestamp::from_seconds(0)).unwrap();
        let amount = Uint128::from(1_000u128);

        // normal case
        let (net, fee) = mgr.tx_fee(amount).unwrap();
        let expected_fee = Decimal::from_ratio(amount, Uint128::one())
            .checked_mul(Decimal::percent(5))
            .unwrap()
            .to_uint_floor();
        assert_eq!(fee, expected_fee);
        assert_eq!(net, amount.checked_sub(expected_fee).unwrap());

        // zero transaction rate
        let mgr_zero = FeeManager::new(make_rates(0, 0, 0), Timestamp::from_seconds(0)).unwrap();
        let (net, fee) = mgr_zero.tx_fee(amount).unwrap();
        assert_eq!(fee, Uint128::zero());
        assert_eq!(net, amount);
    }

    #[test]
    fn perf_fee_gain_and_no_gain() {
        let mut mgr = FeeManager::new(make_rates(0, 20, 0), Timestamp::from_seconds(0)).unwrap();
        // initial high_water_mark = 0
        let gain_value = Uint128::from(500u128);
        let fee = mgr.perf_fee(gain_value).unwrap();
        let expected = Decimal::from_ratio(gain_value, Uint128::one())
            .checked_mul(Decimal::percent(20))
            .unwrap()
            .to_uint_floor();
        assert_eq!(fee, expected);
        // high_water_mark updated
        assert_eq!(mgr.high_water_mark, gain_value);

        // no further gain
        let fee2 = mgr.perf_fee(gain_value).unwrap();
        assert_eq!(fee2, Uint128::zero());
    }
}
