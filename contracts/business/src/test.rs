#![cfg(test)]

// Five tests covering: happy path settlement, an edge-case failure (settling
// with no outstanding balance), and state verification after settlement,
// plus credit-cap enforcement and partial settlement behavior.

use soroban_sdk::{
    testutils::Address as _,
    token, Address, Env,
};

use crate::{SariSettleContract, SariSettleContractClient};

// Helper: deploys a mock USDC token contract (Stellar's built-in token contract)
// and returns its client plus an admin to mint from, so tests have real transferable
// token balances to work with rather than mocking transfer logic.
fn setup_token<'a>(env: &Env, admin: &Address) -> (Address, token::StellarAssetClient<'a>, token::Client<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let asset_client = token::StellarAssetClient::new(env, &contract_address);
    let token_client = token::Client::new(env, &contract_address);
    (contract_address, asset_client, token_client)
}

#[test]
fn test_happy_path_settlement() {
    // Test 1 (Happy path): the MVP transaction executes successfully end-to-end.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env); // Liza, the wholesaler
    let reseller = Address::generate(&env);

    let (usdc_address, usdc_admin, usdc_token) = setup_token(&env, &admin);
    usdc_admin.mint(&reseller, &1_000_i128);

    let contract_id = env.register_contract(None, SariSettleContract);
    let client = SariSettleContractClient::new(&env, &contract_id);

    client.initialize(&admin, &usdc_address);
    client.extend_credit(&reseller, &1_000_i128, &500_i128); // reseller now owes 500

    client.settle(&reseller, &500_i128);

    // Reseller's USDC balance should have decreased by the settled amount.
    assert_eq!(usdc_token.balance(&reseller), 500_i128);
    assert_eq!(usdc_token.balance(&admin), 500_i128);
}

#[test]
#[should_panic(expected = "reseller has no outstanding balance to settle")]
fn test_edge_case_settle_with_no_balance_owed() {
    // Test 2 (Edge case): settling when the reseller owes nothing should fail.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reseller = Address::generate(&env);

    let (usdc_address, usdc_admin, _usdc_token) = setup_token(&env, &admin);
    usdc_admin.mint(&reseller, &1_000_i128);

    let contract_id = env.register_contract(None, SariSettleContract);
    let client = SariSettleContractClient::new(&env, &contract_id);

    client.initialize(&admin, &usdc_address);
    // Extend credit but with zero restock, so owed stays at 0.
    client.extend_credit(&reseller, &1_000_i128, &0_i128);

    // Attempting to settle a non-existent debt should panic.
    client.settle(&reseller, &100_i128);
}

#[test]
fn test_state_verification_after_settlement() {
    // Test 3 (State verification): storage reflects correct state after settlement.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reseller = Address::generate(&env);

    let (usdc_address, usdc_admin, _usdc_token) = setup_token(&env, &admin);
    usdc_admin.mint(&reseller, &1_000_i128);

    let contract_id = env.register_contract(None, SariSettleContract);
    let client = SariSettleContractClient::new(&env, &contract_id);

    client.initialize(&admin, &usdc_address);
    client.extend_credit(&reseller, &1_000_i128, &700_i128); // owed = 700

    client.settle(&reseller, &300_i128); // partial settlement

    let line = client.get_credit_line(&reseller);
    assert_eq!(line.owed, 400_i128);        // 700 - 300
    assert_eq!(line.total_repaid, 300_i128); // lifetime repayment tracked
    assert_eq!(line.cap, 1_000_i128);        // cap unchanged
}

#[test]
#[should_panic(expected = "restock exceeds reseller's credit cap")]
fn test_credit_cap_enforcement() {
    // Additional edge case: extending credit beyond the cap should fail,
    // mirroring how a Stellar trustline limits exposure.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reseller = Address::generate(&env);

    let (usdc_address, _usdc_admin, _usdc_token) = setup_token(&env, &admin);

    let contract_id = env.register_contract(None, SariSettleContract);
    let client = SariSettleContractClient::new(&env, &contract_id);

    client.initialize(&admin, &usdc_address);
    // Cap is 500, but we try to extend 600 worth of restock on credit.
    client.extend_credit(&reseller, &500_i128, &600_i128);
}

#[test]
fn test_multiple_partial_settlements_clear_balance() {
    // Additional happy-path variant: two partial settlements together
    // fully clear the owed balance, confirming repeated MVP usage works.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reseller = Address::generate(&env);

    let (usdc_address, usdc_admin, usdc_token) = setup_token(&env, &admin);
    usdc_admin.mint(&reseller, &1_000_i128);

    let contract_id = env.register_contract(None, SariSettleContract);
    let client = SariSettleContractClient::new(&env, &contract_id);

    client.initialize(&admin, &usdc_address);
    client.extend_credit(&reseller, &1_000_i128, &500_i128); // owed = 500

    client.settle(&reseller, &200_i128); // owed = 300
    client.settle(&reseller, &300_i128); // owed = 0

    let line = client.get_credit_line(&reseller);
    assert_eq!(line.owed, 0_i128);
    assert_eq!(line.total_repaid, 500_i128);
    assert_eq!(usdc_token.balance(&reseller), 500_i128);
}