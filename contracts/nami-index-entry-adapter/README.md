# nami-index-entry-adapter

### Technical Overview
`nami-index-entry-adapter` is a CosmWasm contract that serves as an adapter for interacting with **nami-index-fixed** contract, facilitating deposits and withdrawals with token swaps. It supports multi-step operations, including swapping input tokens to a quote denomination, depositing into an index, and handling residual funds. The contract uses a callback mechanism (`ExecuteMsg::Then`) to chain operations and manage swap contracts via sudo messages.

---

### Instantiate
- Sets the contract name and version using `cw2`.
- Stores the configuration, including the quote denomination (`quote_denom`).
- Registers swap contracts for specified denominations, validating their compatibility with the quote denomination.

---

### Execute

#### `ExecuteMsg::Deposit { index, swaps }`
- Validates that input funds match the quote denomination and amount.
- Processes a list of swap entries, generating swap messages to convert funds to required denominations.
- Ensures the total swapped amount matches the input amount.
- Triggers a callback via `ExecuteMsg::Then(ThenType::Deposit)` to finalize the deposit into the index.

#### `ExecuteMsg::Withdraw { index, min_return }`
- Validates the receipt token amount for the specified index.
- Initiates withdrawal from the index contract.
- Triggers a callback via `ExecuteMsg::Then(ThenType::Swap)` to swap withdrawn assets back to the quote denomination.

#### `ExecuteMsg::Then(then)`
- Restricted to the contract itself for callback execution.
- Handles three callback types:
    - **ThenType::Deposit { sender, index }**:
        - Loads the index and generates a deposit message based on the index’s allocation.
        - Handles residual funds by generating swap messages to convert them to the quote denomination.
        - Triggers a final callback via `ThenType::Send` to return funds to the sender.
    - **ThenType::Swap { sender, min_return }**:
        - Queries the contract’s balances and generates swap messages for all non-zero balances.
        - Triggers a final callback via `ThenType::Send` to return funds to the sender.
    - **ThenType::Send { sender, min_return }**:
        - Sends all contract balances to the sender via `BankMsg::Send`.
        - Validates that the returned amount meets the optional `min_return` requirement.

---

### Sudo

#### `SudoMsg::AddSwapContract { denom, contract }`
- Adds a swap contract for a given denomination, validating that its quote denomination matches the contract’s configuration.
- Stores the contract address in the state.

#### `SudoMsg::RemoveSwapContract { denom }`
- Removes a swap contract for a given denomination from the state.

---

### Query

#### `QueryMsg::Config {}`
- Returns the contract’s configuration, including the quote denomination.

#### `QueryMsg::SwapContracts {}`
- Returns a list of all registered swap contracts (denomination and contract address pairs).

#### `QueryMsg::SwapContract { denom }`
- Returns the contract address for a specific denomination’s swap contract.

---

### Index Behavior
- Interacts with nami-index-fixed to handle deposits and withdrawals.
- Loads index allocation and receipt token denomination dynamically via queries.
- Calculates deposit amounts based on proportional allocation and handles residual funds via swaps.
- Supports withdrawal by burning receipt tokens and swapping withdrawn assets.

---

### Swap Contract Management
- Stores a mapping of denominations to swap contract addresses.
- Validates swap contracts during addition to ensure compatibility with the quote denomination.
- Generates swap messages using the `rujira_rs::fin` interface, supporting optional minimum return and callback parameters.

---

### Error Handling
- **Unauthorized**: Triggered if `ExecuteMsg::Then` is called by an unauthorized sender.
- **InsufficientFunds**: Triggered if input funds are missing or invalid.
- **InvalidSwapData**: Triggered if the total swapped amount does not match the input amount.
- **InsufficientReturn**: Triggered if the returned amount is below the specified `min_return`.
- **SwapContractNotFound**: Triggered if a swap contract is not registered for a denomination.
- **InvalidQuoteDenom**: Triggered if a swap contract’s quote denomination does not match the configuration.
- Standard CosmWasm errors (e.g., overflow, payment errors) and index-specific errors (e.g., invalid deposit proportions) are also handled.
