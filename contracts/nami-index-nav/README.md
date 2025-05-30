# nami-index-nav

### Audit Report
The audit report by Halborn can be found [here](https://www.halborn.com/audits/thorchain/nami-protocol-rujira-index-product-0612c8).


### Technical Overview
`nami-index-nav` is a CosmWasm contract implementing a net asset value (NAV)-based index strategy. Each receipt token represents a proportional claim on the total value of the fund. Deposits and withdrawals are made in `quote_denom` (e.g., USDC), and the fund's NAV is determined using external price feeds. Rebalancing is permissionless and triggered via the `Run` entry point.

---

### Instantiate
- Stores the configuration and fee parameters.
- Initializes the receipt token via Rujira `TokenFactory`.
- Sets up the fee collector.

---

### Execute

#### `ExecuteMsg::Deposit {}`
- Requires exact payment in the configured `quote_denom`.
- Computes current NAV using price feeds.
- Mints receipt tokens proportional to the deposited amount.
- Mints AUM fee tokens to the fee collector.
- Emits a `deposit` event.

#### `ExecuteMsg::Withdraw { slippage }`
- Validates the receipt token amount via `must_pay`.
- Applies the exit fee and calculates the net share.
- Computes NAV to determine the withdrawal amount.
- Withdraws fund assets based on current portfolio allocation.
- Swaps underlying assets to `quote_denom` with the specified slippage tolerance.
- Sends tokens to the user.
- Mints AUM and exit fee tokens to the fee collector.
- Emits a `withdraw` event.

#### `ExecuteMsg::Run {}`
- Triggers a rebalancing operation.
- Calculates target vs actual portfolio weights using oracle prices.
- Executes a set of swap messages to realign the portfolio.
- Callable by anyone.
- Emits a `run` event.

---

### Sudo
#### `SudoMsg::UpdateFees { fee_collector, fees }`
- Updates fee collector address and fee parameters.
#### `SudoMsg::AddAllocation { denom, weight, contract, oracle, threshold }`
- Adds a new asset allocation with specified weight, swap contract, price oracle, and threshold for rebalancing.
#### `SudoMsg::RemoveAllocation { denom }`
- Removes an existing asset allocation.

---

### Vault Behavior
- Computes NAV using real-time prices and current balances using oracle prices.
- Manages portfolio asset distribution and target weights.
- Handles swap logic required during `Run`-based rebalancing.

---

### Receipt Token
- Created during `instantiate`.
- Minted on deposit and burned on withdrawal.
- AUM and exit fees are minted to the fee collector address.

---

### Fee Logic
- **AUM Fee**: Calculated on each interaction based on total supply. Minted to the fee collector to reflect an annualized rate.
- **Exit Fee**: Applied when burning receipt tokens during withdrawals.