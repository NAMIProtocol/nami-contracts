use cosmwasm_std::Event;

pub fn execute_event(contract_addr: String, affiliate: Option<(String, u16)>) -> Event {
    let mut event =
        Event::new("nami-affiliate-execute").add_attribute("contract_addr", contract_addr);
    if let Some((addr, bps)) = affiliate {
        event = event
            .add_attribute("affiliate", addr)
            .add_attribute("bps", bps.to_string());
    } else {
        event = event
            .add_attribute("affiliate", "")
            .add_attribute("bps", "");
    }
    event
}
