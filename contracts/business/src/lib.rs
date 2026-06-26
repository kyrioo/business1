#![no_std]

// SariSettle: on-chain credit-line settlement for sari-sari store wholesalers
// and their resellers. A wholesaler (e.g. Liza) extends a credit line to each
// reseller. Resellers settle what they owe in USDC; the contract deducts the
// owed balance and replenishes available credit, removing manual bookkeeping
// and the need to chase payments by hand.

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol, symbol_short,
};

// ---------- Storage Keys ----------
// We use a typed enum so storage access is explicit and collision-free.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,                 // the wholesaler (e.g. Liza) who manages the contract
    UsdcToken,              // address of the USDC token contract used for settlement
    CreditLine(Address),    // per-reseller credit line state
}

// CreditLine tracks how much credit a reseller has been extended (cap),
// how much of that they currently owe, and how much they've repaid in total.
#[contracttype]
#[derive(Clone)]
pub struct CreditLine {
    pub cap: i128,          // max credit Liza is willing to extend (trustline-style exposure cap)
    pub owed: i128,         // current outstanding balance the reseller must settle
    pub total_repaid: i128, // lifetime repayments, useful for credit-history/trust scoring
}

const SETTLED_EVENT: Symbol = symbol_short!("settled");

#[cfg(test)]
mod test;

#[contract]
pub struct SariSettleContract;

#[contractimpl]
impl SariSettleContract {
    /// Initializes the contract. Called once by the wholesaler (admin) at deploy time.
    /// `admin`      - the wholesaler's address (e.g. Liza), the only one who can extend credit.
    /// `usdc_token` - the Stellar Asset Contract address for USDC, used for all settlements.
    pub fn initialize(env: Env, admin: Address, usdc_token: Address) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
    }

    /// Extends or updates a reseller's credit line (the trustline-style exposure cap).
    /// Only the admin (wholesaler) can call this. Increases `owed` by the amount of
    /// goods just supplied on credit, capped by `cap`.
    pub fn extend_credit(env: Env, reseller: Address, cap: i128, restock_amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::CreditLine(reseller.clone());
        let mut line: CreditLine = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(CreditLine {
                cap,
                owed: 0,
                total_repaid: 0,
            });

        // Always let the admin update the cap (e.g. raise it as trust grows).
        line.cap = cap;

        let new_owed = line.owed + restock_amount;
        // Enforce the exposure cap, mirroring how a Stellar trustline limits exposure.
        if new_owed > line.cap {
            panic!("restock exceeds reseller's credit cap");
        }
        line.owed = new_owed;

        env.storage().persistent().set(&key, &line);
    }

    /// The MVP transaction: a reseller settles part or all of what they owe.
    /// Reseller sends `amount` USDC to the contract (held on behalf of the admin),
    /// the contract verifies the payment, deducts it from `owed`, and records the
    /// repayment. This is the on-chain action that proves the product end-to-end.
    pub fn settle(env: Env, reseller: Address, amount: i128) {
        reseller.require_auth();

        if amount <= 0 {
            panic!("settlement amount must be positive");
        }

        let key = DataKey::CreditLine(reseller.clone());
        let mut line: CreditLine = env
            .storage()
            .persistent()
            .get(&key)
            .expect("no credit line found for this reseller");

        if line.owed <= 0 {
            panic!("reseller has no outstanding balance to settle");
        }

        // Move real USDC from reseller -> admin (the wholesaler) via the token contract.
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let usdc_token: Address = env.storage().instance().get(&DataKey::UsdcToken).unwrap();
        let token_client = token::Client::new(&env, &usdc_token);
        token_client.transfer(&reseller, &admin, &amount);

        // Deduct from owed balance, never going below zero (overpayment is rejected,
        // not silently absorbed, so reseller and wholesaler ledgers always match).
        if amount > line.owed {
            panic!("settlement amount exceeds outstanding balance");
        }
        line.owed -= amount;
        line.total_repaid += amount;

        env.storage().persistent().set(&key, &line);

        // Emit an event so off-chain dashboards (e.g. Liza's app) can update in real time.
        env.events().publish((SETTLED_EVENT, reseller), amount);
    }

    /// Read-only view of a reseller's current credit line. Used by the dashboard
    /// and by tests to verify state after a settlement.
    pub fn get_credit_line(env: Env, reseller: Address) -> CreditLine {
        let key = DataKey::CreditLine(reseller);
        env.storage()
            .persistent()
            .get(&key)
            .expect("no credit line found for this reseller")
    }
}