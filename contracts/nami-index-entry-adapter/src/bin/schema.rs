use cosmwasm_schema::write_api;

use nami_rs::index_entry_adapter;

fn main() {
    write_api! {
        instantiate: index_entry_adapter::InstantiateMsg,
        execute: index_entry_adapter::ExecuteMsg,
        query: index_entry_adapter::QueryMsg,
        sudo: index_entry_adapter::SudoMsg,
    }
}
