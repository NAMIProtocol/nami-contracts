use cosmwasm_schema::write_api;

use nami_rs::index_fixed;

fn main() {
    write_api! {
        instantiate: index_fixed::InstantiateMsg,
        execute: index_fixed::ExecuteMsg,
        query: index_fixed::QueryMsg,
        sudo: index_fixed::SudoMsg,
    }
}
