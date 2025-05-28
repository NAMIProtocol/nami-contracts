use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary};

#[cw_serde]
pub struct InstantiateMsg {
    pub max_affiliate_fee_bps: u16,
    pub whitelist: Option<Vec<String>>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Execute {
        contract_addr: String,
        msg: Binary,
        affiliate: Option<(String, u16)>,
    },
    Send {
        sender: String,
    },
}

#[cw_serde]
pub enum SudoMsg {
    AddWhitelisted { addr: String },
    RemoveWhitelisted { addr: String },
    UpdateConfig { max_affiliate_fee_bps: u16 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Vec<Addr>)]
    Whitelists {},
}
