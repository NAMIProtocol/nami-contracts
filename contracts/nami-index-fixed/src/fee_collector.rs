use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Env, Storage, Uint128};
use cw_storage_plus::Item;
use nami_rs::{FeeManager, FeeRates};

static FEE_MANAGER: Item<FeeManager> = Item::new("fee_manager");

#[cw_serde]
pub struct FeeCollector {}

impl FeeCollector {
    pub fn init(
        storage: &mut dyn Storage,
        env: &Env,
        fee_rates: FeeRates,
    ) -> Result<(), ContractError> {
        FEE_MANAGER.save(storage, &FeeManager::new(fee_rates, env.block.time)?)?;
        Ok(())
    }

    pub fn aum_fee(
        storage: &mut dyn Storage,
        env: &Env,
        total_supply: Uint128,
    ) -> Result<Uint128, ContractError> {
        let mut fee_manager = FEE_MANAGER.load(storage)?;
        let fee_amount = fee_manager.aum_fee(env.block.time, total_supply)?;
        FEE_MANAGER.save(storage, &fee_manager)?;
        Ok(fee_amount)
    }

    pub fn exit_fee(
        storage: &dyn Storage,
        amount: Uint128,
    ) -> Result<(Uint128, Uint128), ContractError> {
        let fee_manager = FEE_MANAGER.load(storage)?;
        let (net, fee) = fee_manager.tx_fee(amount)?;
        Ok((net, fee))
    }
}
