use cosmwasm_schema::write_api;

use nami_rs::index_nav;

fn main() {
    write_api! {
        instantiate: index_nav::InstantiateMsg,
        execute: index_nav::ExecuteMsg,
        query: index_nav::QueryMsg,
        sudo: index_nav::SudoMsg,
    }
}
