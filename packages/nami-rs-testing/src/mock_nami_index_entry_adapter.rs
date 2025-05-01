use cosmwasm_std::{Addr, Coin, StdResult, Uint128};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};
use nami_index_entry_adapter::contract::{execute, instantiate, query, sudo};
use nami_rs::index_entry_adapter::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg, SwapContractResponse,
    SwapContractsResponse, SwapEntry,
};
use rujira_rs_testing::RujiraApp;

pub struct MockNamiIndexEntryAdapter {
    pub address: Addr,
}

impl MockNamiIndexEntryAdapter {
    pub fn new(app: &mut RujiraApp, msg: InstantiateMsg) -> Self {
        let code = Box::new(ContractWrapper::new(execute, instantiate, query).with_sudo(sudo));
        let code_id = app.store_code(code);
        let owner = app.api().addr_make("owner");

        let index_fixed = app
            .instantiate_contract(code_id, owner.clone(), &msg, &[], "IndexEntryAdapter", None)
            .unwrap();

        MockNamiIndexEntryAdapter {
            address: index_fixed,
        }
    }

    pub fn execute_deposit(
        &self,
        app: &mut RujiraApp,
        user: &str,
        funds: Vec<Coin>,
        index: String,
        swaps: Vec<SwapEntry>,
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Deposit { index, swaps },
            &funds,
        )
    }

    pub fn execute_withdraw(
        &self,
        app: &mut RujiraApp,
        user: &str,
        funds: Vec<Coin>,
        index: String,
        min_return: Option<Uint128>,
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Withdraw { index, min_return },
            &funds,
        )
    }

    pub fn sudo_add_swap_contract(
        &self,
        app: &mut RujiraApp,
        denom: String,
        contract: String,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(
            self.address.clone(),
            &SudoMsg::AddSwapContract { denom, contract },
        )
    }

    pub fn sudo_remove_swap_contract(
        &self,
        app: &mut RujiraApp,
        denom: String,
    ) -> anyhow::Result<AppResponse> {
        app.wasm_sudo(self.address.clone(), &SudoMsg::RemoveSwapContract { denom })
    }

    pub fn query_config(&self, app: &mut RujiraApp) -> StdResult<ConfigResponse> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
    }

    pub fn query_swap_contracts(&self, app: &mut RujiraApp) -> StdResult<SwapContractsResponse> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::SwapContracts {})
    }

    pub fn query_swap_contract(
        &self,
        app: &mut RujiraApp,
        denom: String,
    ) -> StdResult<SwapContractResponse> {
        app.wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::SwapContract { denom })
    }
}
