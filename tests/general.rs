use near_contract_standards::storage_management::StorageBalance;
use near_sdk::json_types::U128;
use near_sdk::log;
use near_sdk::serde_json::json;
use near_sdk::{require, AccountId};
use near_sdk_sim::{call, deploy, init_simulator, to_yocto, view, ContractAccount, UserAccount};

extern crate vault_contract;
// Note: the struct xxxxxxContract is created by #[near_bindgen] from near-sdk in combination with
// near-sdk-sim
use vault_contract::ContractContract as VaultContract;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    TOKEN_WASM_BYTES => "./res/vault_contract.wasm",
}

const CONTRACT_ID: &str = "contract";
const CONTRACT_ID_EHT_TESTNET: &str = "eth.fakes.testnet";

// Register the given `user` with Vault contract
pub fn register_user(user: &near_sdk_sim::UserAccount, value_to_deposit: u32) {
  let outcome = user.call(
    CONTRACT_ID.parse().unwrap(),
    "storage_deposit",
    &json!({
        "account_id": user.account_id(),
        "registration_only": false
    })
    .to_string()
    .into_bytes(),
    near_sdk_sim::DEFAULT_GAS / 2,
    to_yocto(&value_to_deposit.to_string()), // attached deposit
  );

  outcome.assert_success();
}
pub fn init_one(
  initial_balance: u32,
) -> (UserAccount, ContractAccount<VaultContract>, UserAccount) {
  let mut genesis = near_sdk_sim::runtime::GenesisConfig::default();
  genesis.gas_price = 0;
  genesis.gas_limit = u64::MAX;
  let master_account = init_simulator(Some(genesis));
  // uses default values for deposit and gas

  let owner_id = master_account.account_id();
  let vault_shares: U128 = U128(0);

  let vault = deploy!(
    // Contract Proxy
    contract: VaultContract,
    // Contract account id
    contract_id: CONTRACT_ID,
    // Bytes of contract
    bytes: &TOKEN_WASM_BYTES,
    // User deploying the contract,
    signer_account: master_account,
    // init method
    init_method: new(owner_id, vault_shares)
  );
  let alice = master_account.create_user(
    AccountId::new_unchecked("alice".to_string()),
    to_yocto(&initial_balance.to_string()),
  );
  register_user(&alice, initial_balance / 2);
  register_user(&master_account, initial_balance / 2);

  (master_account, vault, alice)
}

#[test]
fn simulate_get_and_extend_whitelisted_tokens() {
  let (_, vault, alice) = init_one(10);

  // TODO: check that after initialization the Vault doesn't have whitelisted tokens
  let outcome = call!(alice, vault.get_whitelisted_tokens(), deposit = 1);
  outcome.assert_success();
  assert_eq!(outcome.logs()[0], "Hello world");

  // step 2: extend whitelisted tokens
  call!(
    alice,
    vault.extend_whitelisted_tokens(vec![CONTRACT_ID_EHT_TESTNET.parse().unwrap()]),
    deposit = 1
  )
  .assert_success();

  // step 3: assert that the token has been added to the Vault
  let outcome2 = call!(alice, vault.get_whitelisted_tokens(), deposit = 1);
  outcome2.assert_success();

  let c: Vec<String> = outcome2.unwrap_json();
  assert_eq!(CONTRACT_ID_EHT_TESTNET, c[0]);
}

#[test]
fn simulate_near_to_wrap() {
  let (root, vault, alice) = init_one(10);

  let outcome = call!(
    root,
    vault.near_to_wrap(root.account_id(), alice.account_id(), "".to_string()),
    deposit = 1
  );

  outcome.assert_success();
}

#[test]
fn fail_near_to_wrap() {
  let (root, vault, alice) = init_one(10);

  // create a new account
  let bob = root.create_user(AccountId::new_unchecked("bob".to_string()), to_yocto("100"));

  // call near_to_wrap without registering bob into the Vault
  let outcome = call!(
    root,
    vault.near_to_wrap(bob.account_id(), alice.account_id(), "".to_string()),
    deposit = 1
  );

  require!(!outcome.is_ok());
}
