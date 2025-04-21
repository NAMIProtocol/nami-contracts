use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};
use rujira_rs::CallbackData;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {
        yield_protocol: Addr,
        callback: Option<CallbackData>,
    },
    Withdraw {
        yield_protocol: Addr,
        callback: Option<CallbackData>,
    },
    Then {
        callback: Option<CallbackData>,
        sender: Addr,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Decimal)]
    Ratio { yield_protocol: Addr },
}
