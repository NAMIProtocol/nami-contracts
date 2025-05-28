use cosmwasm_std::{Addr, Coin, StdResult, Uint128};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};
use nami_index_fixed::contract::{execute, instantiate, query, sudo};
use nami_rs::{
    index_fixed::{
        ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg, VaultStatusResponse,
    },
    FeeManager, FeeRates,
};
use rujira_rs_testing::RujiraApp;

pub struct MockNamiIndexFixed {
    pub address: Addr,
}

impl MockNamiIndexFixed {
    pub fn new(app: &mut RujiraApp, msg: InstantiateMsg) -> Self {
        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let owner = app.api().addr_make("owner");

        let index_fixed = app
            .instantiate_contract(code_id, owner.clone(), &msg, &[], "IndexFixed", None)
            .unwrap();

        MockNamiIndexFixed {
            address: index_fixed,
        }
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
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Withdraw {},
            &funds,
        )
    }

    pub fn execute_run(&self, app: &mut RujiraApp, user: &str) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(user_addr, self.address.clone(), &ExecuteMsg::Run {}, &[])
    }

    pub fn sudo_reallocate(
        &self,
        app: &mut RujiraApp,
        from: &str,
        to: &str,
        weight: Uint128,
        min_return: Option<Uint128>,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(
            self.address.clone(),
            &SudoMsg::Reallocate {
                from: from.to_string(),
                to: to.to_string(),
                weight,
                min_return,
            },
        )
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

    pub fn sudo_add_allocation(
        &self,
        app: &mut RujiraApp,
        allocation: (String, String),
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(
            self.address.clone(),
            &SudoMsg::AddAllocation {
                denom: allocation.0,
                contract: allocation.1,
            },
        )
    }

    pub fn sudo_remove_allocation(
        &self,
        app: &mut RujiraApp,
        denom: String,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(self.address.clone(), &SudoMsg::RemoveAllocation { denom })
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
