use cosmwasm_schema::write_api;

use nami_rs::affiliate;

fn main() {
    write_api! {
        instantiate: affiliate::InstantiateMsg,
        execute: affiliate::ExecuteMsg,
        query: affiliate::QueryMsg,
        sudo: affiliate::SudoMsg,
    }
}
