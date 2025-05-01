use std::ops::{Deref, DerefMut};

use cosmwasm_std::{Addr, Coin, QuerierWrapper, Uint128};
use rujira_rs_testing::RujiraApp;

pub struct NamiApp {
    pub app: RujiraApp,
}

impl NamiApp {
    // Constructor to initialize NamiApp
    pub fn new(app: RujiraApp) -> Self {
        NamiApp { app }
    }

    // Method to query the balance of an address and denom
    pub fn query_balance(&self, address: &str, denom: &str, convert: bool) -> Uint128 {
        let addr = if convert {
            self.app.api().addr_make(address).to_string()
        } else {
            address.to_string()
        };
        self.app.wrap().query_balance(&addr, denom).unwrap().amount
    }

    pub fn query_all_balances(&self, address: &str, convert: bool) -> Vec<Coin> {
        let addr = if convert {
            self.app.api().addr_make(address).to_string()
        } else {
            address.to_string()
        };
        self.app.wrap().query_all_balances(&addr).unwrap()
    }

    pub fn add_balance(&mut self, address: &str, coins: Vec<Coin>, convert: bool) {
        let addr = if convert {
            self.app.api().addr_make(address)
        } else {
            Addr::unchecked(address)
        };
        self.app.init_modules(|router, _, storage| {
            router.bank.init_balance(storage, &addr, coins).unwrap();
        });
    }

    pub fn wrap(&self) -> QuerierWrapper {
        self.app.wrap()
    }
}

impl Deref for NamiApp {
    type Target = RujiraApp;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}

impl DerefMut for NamiApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.app
    }
}
