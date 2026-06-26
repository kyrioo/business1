# SariSettle

On-chain credit-line settlement for sari-sari store wholesalers and their resellers in the Philippines, built on Stellar/Soroban.

## Problem

Liza runs a small sari-sari store network in Cebu, Philippines, supplying 12 neighborhood resellers on credit. She spends ~3 days a month chasing payments via GCash and cash, losing roughly 8% of margin to remittance/cash-handling fees and bad debt from resellers who "forget" what they owe.

## Solution

Liza extends each reseller a credit line tracked on-chain. Resellers settle what they owe by sending USDC directly to the contract at point-of-restock; the contract automatically deducts the owed balance, updates lifetime repayment history, and re-opens credit room for the next restock. Stellar's sub-cent fees and 3–5 second settlement make daily micro-settlement economically viable — something traditional bank transfers or GCash fees would otherwise erase the margin on. Per-reseller exposure caps mirror how Stellar trustlines limit counterparty risk.

## Timeline

- **Day 1:** Contract design, storage model, `initialize` / `extend_credit` functions
- **Day 2:** `settle` function with real USDC token transfer integration, event emission
- **Day 3:** Test suite, testnet deployment, dashboard demo wiring
- **Day 4:** Polish, anchor integration (optional edge), demo rehearsal

## Stellar Features Used

- **USDC transfers** — real settlement currency between reseller and wholesaler
- **Custom tokens** — SARI-CREDIT, a non-transferable on-chain representation of each reseller's credit line
- **Soroban smart contracts** — all credit logic, balance tracking, and auto-deduction
- **Trustlines** — conceptual model for per-reseller exposure caps, enforced in contract logic

## Vision and Purpose

Millions of micro-retail relationships across Southeast Asia run on informal, unenforceable credit. SariSettle gives wholesalers an auditable, automatic ledger and gives resellers a transparent, disputable record of what they owe — without requiring either party to trust a bank, a notebook, or each other's memory. The long-term vision is a reusable credit-settlement primitive any informal supply chain (not just sari-sari stores) can issue against.

## Prerequisites

- Rust (stable, 1.74+) with the `wasm32-unknown-unknown` target installed
- Soroban CLI v21.x or later (`stellar` CLI is also compatible)
- A funded Stellar testnet account for deployment

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli
```

## How to Build

```bash
soroban contract build
```

This produces an optimized `.wasm` file at `target/wasm32-unknown-unknown/release/sari_settle.wasm`.

## How to Test

```bash
cargo test
```

Runs all 5 tests: happy-path settlement, no-balance-owed failure, post-settlement state verification, credit-cap enforcement, and multi-partial-settlement balance clearing.

## How to Deploy to Testnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/sari_settle.wasm \
  --source <YOUR_SOURCE_ACCOUNT> \
  --network testnet
```

This returns a contract ID, e.g. `CABCDEF...`, used in all subsequent invocations.

## Sample CLI Invocation

Initialize the contract (run once, by the wholesaler):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source liza \
  --network testnet \
  -- initialize \
  --admin <LIZA_ADDRESS> \
  --usdc_token <USDC_TESTNET_CONTRACT_ID>
```

Extend credit to a reseller (run by the wholesaler at restock time):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source liza \
  --network testnet \
  -- extend_credit \
  --reseller <RESELLER_ADDRESS> \
  --cap 1000 \
  --restock_amount 500
```

Settle a balance (run by the reseller, the MVP transaction):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source reseller_juan \
  --network testnet \
  -- settle \
  --reseller <RESELLER_ADDRESS> \
  --amount 500
```

## License

MIT