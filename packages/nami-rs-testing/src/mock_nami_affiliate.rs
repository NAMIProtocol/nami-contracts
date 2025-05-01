use cosmwasm_std::{Addr, Coin};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};
use nami_affiliate::contract::{execute, instantiate, query};
use nami_rs::affiliate::{ExecuteMsg, InstantiateMsg};
use rujira_rs_testing::RujiraApp;

pub struct MockNamiAffiliate {
    pub address: Addr,
}

impl MockNamiAffiliate {
    pub fn new(app: &mut RujiraApp, msg: InstantiateMsg) -> Self {
        let code = Box::new(ContractWrapper::new(execute, instantiate, query));
        let code_id = app.store_code(code);
        let owner = app.api().addr_make("owner");

        let affiliate = app
            .instantiate_contract(code_id, owner.clone(), &msg, &[], "NamiAffiliate", None)
            .unwrap();

        MockNamiAffiliate { address: affiliate }
    }

    pub fn execute(
        &self,
        app: &mut RujiraApp,
        user: &str,
        msg: ExecuteMsg,
        funds: &[Coin],
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(user_addr, self.address.clone(), &msg, funds)
    }

    pub fn execute_send(
        &self,
        app: &mut RujiraApp,
        user: &str,
        funds: &[Coin],
    ) -> anyhow::Result<AppResponse> {
        let user_addr = app.api().addr_make(user);
        app.execute_contract(
            user_addr,
            self.address.clone(),
            &ExecuteMsg::Send {
                sender: user.to_string(),
            },
            funds,
        )
    }
}
