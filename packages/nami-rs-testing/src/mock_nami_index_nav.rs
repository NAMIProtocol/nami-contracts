use cosmwasm_std::{Addr, Coin, Decimal, StdResult};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};
use nami_index_nav::contract::{execute, instantiate, query, sudo};
use nami_rs::{
    index_nav::{
        ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg, VaultStatusResponse,
    },
    AssetAllocation, FeeManager, FeeRates, OracleConfig,
};
use rujira_rs_testing::RujiraApp;

pub struct MockNamiIndexNav {
    pub address: Addr,
}

impl MockNamiIndexNav {
    pub fn new(app: &mut RujiraApp, msg: InstantiateMsg) -> anyhow::Result<Self> {
        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let owner = app.api().addr_make("owner");

        let index_nav =
            app.instantiate_contract(code_id, owner.clone(), &msg, &[], "IndexNav", None)?;

        Ok(MockNamiIndexNav { address: index_nav })
    }

    pub fn execute_deposit(
        &self,
        app: &mut RujiraApp,
        user: &str,
        funds: Vec<Coin>,
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Deposit {},
            &funds,
        )
    }

    pub fn execute_withdraw(
        &self,
        app: &mut RujiraApp,
        user: &str,
        funds: Vec<Coin>,
        slippage: Option<Decimal>,
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Withdraw { slippage },
            &funds,
        )
    }

    pub fn execute_run(&self, app: &mut RujiraApp, user: &str) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(user_addr, self.address.clone(), &ExecuteMsg::Run {}, &[])
    }

    pub fn sudo_set_fees(
        &self,
        app: &mut RujiraApp,
        fee_collector: Option<String>,
        fees: FeeRates,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(
            self.address.clone(),
            &SudoMsg::UpdateFees {
                fee_collector,
                fees,
            },
        )
    }

    pub fn sudo_update_allocation(
        &self,
        app: &mut RujiraApp,
        allocation: Vec<AssetAllocation<OracleConfig>>,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(self.address.clone(), &SudoMsg::UpdateAllocation(allocation))
    }

    pub fn query_config(&self, app: &mut RujiraApp) -> StdResult<ConfigResponse> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
    }

    pub fn query_status(&self, app: &mut RujiraApp) -> StdResult<VaultStatusResponse> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Status {})
    }

    pub fn query_fees(&self, app: &mut RujiraApp) -> StdResult<FeeManager> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Fees {})
    }
}
