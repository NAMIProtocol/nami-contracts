use cosmwasm_std::{Addr, Coin, Event, Uint128};

pub fn event_deposit(owner: Addr, funds: Vec<Coin>, minted: Uint128) -> Event {
    let funds_str = funds
        .into_iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .collect::<Vec<_>>()
        .join(",");

    Event::new(format!("{}/deposit", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("funds", funds_str)
        .add_attribute("minted", minted)
}

pub fn event_withdraw(owner: Addr, funds: Vec<Coin>, shares: Uint128) -> Event {
    let funds_str = funds
        .into_iter()
        .map(|c| format!("{}{}", c.amount, c.denom))
        .collect::<Vec<_>>()
        .join(",");

    Event::new(format!("{}/withdraw", env!("CARGO_PKG_NAME")))
        .add_attribute("owner", owner)
        .add_attribute("funds", funds_str)
        .add_attribute("shares", shares)
}

pub fn event_reallocate(from: String, to: String, weight: Uint128) -> Event {
    Event::new(format!("{}/reallocate", env!("CARGO_PKG_NAME")))
        .add_attribute("from", from)
        .add_attribute("to", to)
        .add_attribute("weight", weight)
}
