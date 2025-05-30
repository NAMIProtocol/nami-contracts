# nami-affiliate

### Audit Report
The audit report by Halborn can be found [here](https://www.halborn.com/audits/thorchain/nami-protocol-rujira-index-product-0612c8).


### Technical Overview
`nami-affiliate` is a CosmWasm contract designed to facilitate affiliate fee distribution for transactions. It supports executing messages to a target contract while optionally deducting affiliate fees from the input funds. The contract ensures fees are calculated based on a basis points (bps) system and sent to an affiliate address, with the remaining funds forwarded to the target contract. It also includes a callback mechanism to return residual balances to the sender.

---

### Instantiate
- Sets the contract name and version using `cw2`.
- Initializes with no additional configuration.

---

### Execute

#### `ExecuteMsg::Execute { contract_addr, msg, affiliate }`
- Validates that input funds are non-empty.
- If an affiliate is specified (address and bps), calculates the fee (up to 100% or 10,000 bps) for each denomination.
- Splits funds into net funds (for the target contract) and fees (for the affiliate).
- Sends fees to the affiliate address via `BankMsg::Send` if non-zero.
- Forwards net funds and the provided message to the target contract via `WasmMsg::Execute`.
- Triggers a callback to itself via `ExecuteMsg::Send` to handle residual balances.
- Emits an `nami-affiliate-execute` event with contract address and affiliate details.

#### `ExecuteMsg::Send { sender }`
- Restricted to the contract itself (ensures callback authenticity).
- Queries the contract’s current balances.
- Validates that balances are non-empty.
- Sends all balances to the original sender via `BankMsg::Send`.
- Used to return residual funds after execution.

---

### Query
- Supports a placeholder query (`QueryMsg`) that returns an empty response.
- No state queries are currently implemented.

---

### Fee Logic
- **Affiliate Fee**: Calculated per denomination as a percentage (bps) of input funds, capped at 10,000 bps (100%).
- Fees are deducted from input funds and sent to the affiliate address.
- Net funds (input minus fees) are forwarded to the target contract.
- Residual balances (if any) are returned to the sender via the `Send` callback.

---

### Event
- **nami-affiliate-execute**: Emitted on `ExecuteMsg::Execute`. Includes:
    - `contract_addr`: Target contract address.
    - `affiliate`: Affiliate address (or empty if none).
    - `bps`: Basis points for the fee (or empty if no affiliate).

---

### Error Handling
- **Unauthorized**: Triggered if `ExecuteMsg::Send` is called by an unauthorized sender.
- **InsufficientFunds**: Triggered if no funds are provided in `ExecuteMsg::Execute`.
- **InvalidAffiliateFee**: Triggered if the affiliate fee exceeds 10,000 bps.
- **InvalidAffiliateCall**: Triggered if `ExecuteMsg::Send` is called with no balances to send.
- Standard CosmWasm errors (e.g., overflow, division by zero) are also handled.
