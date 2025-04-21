use cosmwasm_std::{Addr, Event, Uint128};

pub fn event_deposit(owner: Addr, amount: Uint128, minted: Uint128) -> Event {
    Event::new(format!("{}/deposit", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("minted", minted)
}

pub fn event_withdraw(owner: Addr, amount: Uint128, shares: Uint128) -> Event {
    Event::new(format!("{}/withdraw", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
}

pub fn event_run(owner: Addr) -> Event {
    Event::new(format!("{}/run", env!("CARGO_PKG_NAME"))).add_attribute("owner", owner)
}
