# nami-index-fixed

### Audit Report
The audit report by Halborn can be found [here](https://www.halborn.com/audits/thorchain/nami-protocol-rujira-index-product-0612c8).


### Technical Overview
`nami-index-fixed` is a CosmWasm contract implementing a fixed-unit index strategy. Each receipt token represents a deterministic basket of tokens. The contract supports exact-ratio deposits, proportional withdrawals, and a reallocation entry point protected by `sudo`. Reallocation is assumed to be a two-leg token swap. For example, swapping Token A to Token B is done via `Token A -> base_token -> Token B`.

---

### Instantiate
- Stores the configuration, fee setup, and target allocation.
- Initializes the receipt token via Rujira `TokenFactory`.
- Sets up the vault and fee collector.

---

### Execute

#### `ExecuteMsg::Deposit {}`
- Validates that input funds match the exact ratio (denominations and amounts).
- Updates internal vault balances.
- Mints receipt tokens.
- Mints AUM fee tokens to the fee collector.
- Emits a `deposit` event.

#### `ExecuteMsg::Withdraw {}`
- Validates the receipt token amount via `must_pay`.
- Updates internal vault balances.
- Applies the exit fee and calculates the user’s net share.
- Burns receipt tokens.
- Withdraws the corresponding basket assets from the vault.
- Sends funds to the user.
- Mints AUM and exit fee tokens to the fee collector.
- Emits a `withdraw` event.

#### `ExecuteMsg::Callback(cb)`
- Only callback supported: `AfterReallocate { swap_to, min_return }`.
- Finalizes reallocation by executing the second leg of the token swap (from `base_token` to `swap_to`).

#### `ExecuteMsg::Run {}`
- Triggers rebalancing of the vault after token swaps (used internally).

---

### Sudo

#### `SudoMsg::Reallocate { from, to, weight, min_return }`
- Validates inputs and calculates current total supply.
- Adjusts internal allocation weights.
- Triggers the first leg of the token reallocation (from `from` to `base_token`) with `min_return`.
- Emits a `reallocate` event.

#### `SudoMsg::UpdateFees { fee_collector, fees }`
- Updates fee parameters and fee collector address.

#### `SudoMsg::RemoveAllocation { denom }`
- Removes an allocation for the given denomination.

#### `SudoMsg::AddAllocation { denom, contract }`
- Adds a new allocation and rebalances vault with existing supply.

---

### Vault Behavior
- Tracks internal balances per denomination.
- Only allows deposits that exactly match a proportion of the defined allocation.
- Supports pro-rata withdrawals based on receipt token amount.
- Manual reallocation is initiated via `sudo` and finalized via `Callback`.

---

### Receipt Token
- Created during `instantiate`.
- Minted on deposit and burned on withdrawal.
- AUM and exit fees are minted to the fee collector address.

---

### Fee Logic
- **AUM Fee**: Calculated on every interaction based on the total supply. Fee is minted to the fee collector, targeting an annualized rate.
- **Exit Fee**: Taken from the user’s receipt tokens on withdrawal.