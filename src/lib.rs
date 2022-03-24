use std::collections::HashMap;
use std::convert::Into;
use std::convert::TryInto;
use std::fmt;

use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near_bindgen, AccountId, BorshStorageKey, Gas, Balance,
    PanicOnDefault, Promise, PromiseResult,
};

use crate::account_deposit::{Account, VAccount};
mod account_deposit;
mod storage_impl;
mod token_receiver;

/// Single swap action.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SwapAction {
    /// Pool which should be used for swapping.
    pub pool_id: u64,
    /// Token to swap from.
    pub token_in: AccountId,
    /// Amount to exchange.
    /// If amount_in is None, it will take amount_out from previous step.
    /// Will fail if amount_in is None on the first step.
    pub amount_in: Option<U128>,
    /// Token to swap into.
    pub token_out: AccountId,
    /// Required minimum amount of token_out.
    pub min_amount_out: U128,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
    UserShares,
    VaultShares,
    Whitelist,
    AccountTokens { account_id: AccountId },
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct RefStorageState {
    pub deposit: U128,
    pub usage: U128,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running,
    Paused,
}

impl fmt::Display for RunningState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunningState::Running => write!(f, "Running"),
            RunningState::Paused => write!(f, "Paused"),
        }
    }
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    user_shares: HashMap<AccountId, u128>,
    vault_shares: u128,
    accounts: LookupMap<AccountId, VAccount>,
    whitelisted_tokens: UnorderedSet<AccountId>,
    state: RunningState,
    last_reward_amount: HashMap<String, u128>,
}

// Contracts addresses.
const CONTRACT_ID_REF_TESTNET: &str = "ref.fakes.testnet";
const CONTRACT_ID_REF_EXC: &str = "exchange.ref-dev.testnet";
const CONTRACT_ID_FARM: &str = "farm.leopollum.testnet";
const CONTRACT_ID_WRAP_TESTNET: &str = "wrap.testnet";
const CONTRACT_ID_EHT_TESTNET: &str = "eth.fakes.testnet";
const CONTRACT_ID_DAI_TESTNET: &str = "dai.fakes.testnet";


// Ref exchange functions that we need to call inside the vault.
#[ext_contract(ext_exchange)]
pub trait RefExchange {
    fn exchange_callback_post_withdraw(&mut self, token_id: AccountId, sender_id: AccountId, amount: U128);
    fn get_pool_shares(&mut self, pool_id: u64, account_id: AccountId);
    fn metadata(&mut self);
    fn storage_deposit(&mut self, account_id: AccountId);
    fn get_deposits(&mut self, account_id: AccountId);
    fn add_liquidity(&mut self, pool_id: u64, amounts: Vec<U128>, min_amounts: Option<Vec<U128>>);
    fn swap(&mut self, actions: Vec<SwapAction>, referral_id: Option<AccountId>);
    fn mft_transfer_call(&mut self, receiver_id: AccountId, token_id: String, amount: U128, msg: String);
    fn remove_liquidity(&mut self, pool_id: u64, shares: U128, min_amounts: Vec<U128>);
    fn withdraw(&mut self, token_id: String, amount: U128, unregister: Option<bool>);
}

// Ref farm functions that we need to call inside the vault.
#[ext_contract(ext_farm)]
pub trait FluxusFarming {
    fn mft_transfer_call(&mut self, receiver_id: AccountId, token_id: String, amount: U128, msg: String);
    fn claim_reward_by_seed(&mut self, seed_id: String);
    fn withdraw_seed(&mut self, seed_id: String, amount: U128, msg: String);
    fn withdraw_reward(&mut self, token_id: String, amount: U128, unregister: String);
    fn get_reward(&mut self, account_id: AccountId, token_id: AccountId);
}

// Wrap.testnet functions that we need to call inside the vault.
#[ext_contract(ext_wrap)]
pub trait Wrapnear {
    fn storage_deposit(&mut self);
    fn near_deposit(&mut self);
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: String, msg: String);
    fn near_withdraw(&mut self, amount: U128);
}

// Vault functions that we need to call like a callback.
#[ext_contract(ext_self)]
pub trait VaultContract {
    fn callback_stake_liquidity(&mut self, account_id: AccountId, vault_contract: AccountId) -> Vec<U128>;
    fn callback_update_user_balance(&mut self, account_id: AccountId) -> String;
    fn call_get_pool_shares(&mut self, pool_id: u64, account_id: AccountId) -> String;
    fn callback_withdraw_rewards(&mut self, token_id: String) -> String;
    fn swap_to_withdraw_all(&mut self);
    fn callback_to_withdraw(&mut self);
    fn callback_to_near_withdraw(&mut self, account_id: AccountId);
    fn callback_stake(&mut self, account_id: AccountId);
    fn swap_to_auto(&mut self, farm_id: String);
    fn callback_to_balance(&mut self);
    fn stake_and_liquidity_auto(&mut self, account_id: AccountId, vault_contract: AccountId);
    fn balance_actualization(&mut self, vec: HashMap<AccountId, u128>, shares: String);
    fn add_near_balance(&mut self, account_id: AccountId, amount_available: u128);
    fn sub_near_balance(&mut self, account_id: AccountId, amount_available: u128);
}

#[ext_contract(ext_reffakes)]
pub trait ExtRefFakes {
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: String, msg: String);
}

#[near_bindgen]
impl Contract {
    /// Function that initialize the contract.
    ///
    /// Arguments:
    ///
    /// - `owner_id` - the account id that owns the contract
    /// - `user_shares` - key value pair for account id and number of shares added
    /// - `vault_shares` - the number of shares the Vault starts/has
    /// - `accounts` - lookup map for the registered accounts in the Vault
    /// - `whitelisted_tokens` - the tokens allowed to be used in the Vault
    /// - `state` - keep tracks of the contract state
    #[init]
    pub fn new(owner_id: AccountId, vault_shares: u128) -> Self {
        Self {
            owner_id: owner_id,
            user_shares: HashMap::new(),
            last_reward_amount: HashMap::new(),
            vault_shares,
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            state: RunningState::Running,
        }
    }

    /// Returns the number of shares some accountId has in the Vault
    pub fn get_user_shares(&self, account_id: AccountId) -> Option<String> {
        let user_shares = self.user_shares.get(&account_id);
        if let Some(account) = user_shares {
            Some(account.to_string())
        } else {
            None
        }
    }

    /// Extend the whitelist of tokens.
    #[payable]
    pub fn extend_whitelisted_tokens(&mut self, tokens: Vec<AccountId>) {
        assert_eq!(env::predecessor_account_id(), self.owner_id,"ERR_NOT_ALLOWED");
        for token in tokens {
            self.whitelisted_tokens.insert(&token);
        }
    }

    /// Return the whitelisted tokens.
    pub fn get_whitelisted_tokens(&self) -> Vec<AccountId> {
        self.whitelisted_tokens.to_vec()
    }

    /// Function that return the user`s near storage.
    pub fn get_user_storage_state(&self, account_id: AccountId) -> Option<RefStorageState> {
        assert_eq!(env::predecessor_account_id(), account_id, "ERR_NOT_ALLOWED");
        let acc = self.internal_get_account(&account_id);
        if let Some(account) = acc {
            Some(RefStorageState {
                deposit: U128(account.near_amount),
                usage: U128(account.storage_usage()),
            })
        } else {
            None
        }
    }

    /// Call the ref get_pool_shares function.
    #[private]
    pub fn call_get_pool_shares(&self, pool_id: u64, account_id: AccountId) -> Promise {
        ext_exchange::get_pool_shares(
            pool_id,
            account_id,
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            0,                                    // yocto NEAR to attach
            Gas(10_000_000_000_000),              // gas to attach
        )
    }

    /// Call the ref metadata function.
    pub fn call_meta(&self) -> Promise {
        ext_exchange::metadata(
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            0,                                    // yocto NEAR to attach
            Gas(3_000_000_000_000),               // gas to attach
        )
    }

    /// Call the ref user_register function.
    pub fn call_user_register(&self, account_id: AccountId) -> Promise {
        ext_exchange::storage_deposit(
            account_id,
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            10000000000000000000000,              // yocto NEAR to attach
            Gas(3_000_000_000_000),               // gas to attach
        )
    }

    /// Call the ref get_deposits function.
    fn call_get_deposits(&self, account_id: AccountId) -> Promise {
        ext_exchange::get_deposits(
            account_id,
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(15_000_000_000_000),              // gas to attach
        )
    }

    /// Transfer lp tokens to ref-exchange then swap the amount the contract has in the exchange
    #[payable]
    pub fn auto_function_1(&mut self, farm_id: String) {
        /* TODO:
            a) Add callback to handle failed txs
            b) Send all tokens to exchange, instead of 0.01 each iteration
        */
        self.check_autocompounds_caller(env::predecessor_account_id());

        ext_reffakes::ft_transfer_call(
            CONTRACT_ID_REF_EXC.parse().unwrap(), // receiver_id,
            self.last_reward_amount.get(&farm_id).unwrap().to_string(), //Amount after withdraw the rewards
            "".to_string(),
            CONTRACT_ID_REF_TESTNET.parse().unwrap(),
            1,                                    // yocto NEAR to attach
            Gas(45_000_000_000_000),              // gas to attach (between 40 and 60)
        )
        // Get vault's deposit
        .then(ext_exchange::get_deposits(
            env::current_account_id().try_into().unwrap(),
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(9_000_000_000_000),               // gas to attach
        ))
        // Swap ref tokens and atualize the reward amount
        .then(ext_self::swap_to_auto(
            farm_id,
            env::current_account_id(),           // contract account id
            0,                                   // yocto NEAR to attach
            Gas(41_500_000_000_000),             // gas to attach
        ));
    }

    /// Get amount of tokens available then stake it
    #[payable]
    pub fn auto_function_2(&mut self) {

        self.check_autocompounds_caller(env::predecessor_account_id());

        ext_exchange::get_deposits(
            env::current_account_id().try_into().unwrap(),
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(9_000_000_000_000),               // gas to attach
        )
        // Add liquidity and stake once again
        .then(ext_self::stake_and_liquidity_auto(
            env::current_account_id().try_into().unwrap(),
            env::current_account_id().try_into().unwrap(),
            env::current_account_id(), // vault contract id
            970000000000000000000,     // yocto NEAR to attach
            Gas(200_000_000_000_000),  // gas to attach
        ));
    }

    /// Function to claim the reward from the farm contract
    #[payable]
    pub fn withdraw_of_reward(&mut self) {
        let token_id = CONTRACT_ID_REF_TESTNET.parse().unwrap();

        self.check_autocompounds_caller(env::predecessor_account_id());

        ext_farm::get_reward(
            env::current_account_id().try_into().unwrap(),
            CONTRACT_ID_REF_TESTNET.parse().unwrap(),
            CONTRACT_ID_FARM.parse().unwrap(), // contract account id
            1,                                 // yocto NEAR to attach
            Gas(3_000_000_000_000),            // gas to attach
        )
        .then(ext_self::callback_withdraw_rewards(
            token_id,
            env::current_account_id(),
            1,
            // obs: pass exactly 190
            Gas(190_000_000_000_000),
        ));
    }

    /// Function to claim the reward from the farm contract
    #[payable]
    pub fn claim_reward(&mut self) {

        let seed_id = "exchange.ref-dev.testnet@193".to_string();

        self.check_autocompounds_caller(env::predecessor_account_id());

        ext_farm::claim_reward_by_seed(
            seed_id,
            CONTRACT_ID_FARM.parse().unwrap(), // contract account id
            0,                                 // yocto NEAR to attach
            Gas(40_000_000_000_000),           // gas to attach//was 40?
        );
    }

    fn check_autocompounds_caller(&mut self, account: AccountId){
        if ((account != self.owner_id) 
        && (account.to_string() !=  "goldmine.testnet")
        && (account.to_string() != "hobbyhodlrtest.testnet")
        && (account != env::current_account_id())){
            assert!(false, "ERR_NOT_ALLOWED");
        }
    }
    /// Auto-compound function.
    ///
    /// Responsible to add liquidity and stake.
    #[private]
    #[payable]
    pub fn stake_and_liquidity_auto(&mut self, account_id: AccountId, vault_contract: AccountId) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) =
                    near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens)
                {
                    is_tokens
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };
        let pool_id_to_add_liquidity = 193;
        let token_out1 = CONTRACT_ID_EHT_TESTNET;
        let token_out2 = CONTRACT_ID_DAI_TESTNET;
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {
                quantity_of_token1 = *val
            };
            if key.to_string() == token_out2 {
                quantity_of_token2 = *val
            };
        }
        let pool_id: u64 = 193;

        // Add liquidity
        self.call_add_liquidity(
            pool_id_to_add_liquidity,
            vec![quantity_of_token2, quantity_of_token1],
            None,
        )
        // Get the shares
        .then(ext_exchange::get_pool_shares(
            pool_id,
            account_id.clone().try_into().unwrap(),
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            0,                                    // yocto NEAR to attach
            Gas(10_000_000_000_000),              // gas to attach
        ))
        // Update user balance
        .then(ext_self::callback_to_balance(
            env::current_account_id(),
            0,
            Gas(15_000_000_000_000),
        ))
        .then(ext_self::callback_stake(
            account_id.clone(),
            env::current_account_id(),
            0,
            Gas(90_000_000_000_000),
        ));
    }

    /// Read shares for each account registered.
    #[private]
    pub fn callback_to_balance(&mut self) -> String {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        // If new_shares_quantity > 0:
        if shares.parse::<u128>().unwrap() > 0 {
            let mut vec: HashMap<AccountId, u128> = HashMap::new();

            for (account, val) in self.user_shares.iter() {
                vec.insert(account.clone(), *val);
            }

            /*  TODO: improve or justify the difficulty of iterating then inserting
                Cant write everything here because it is not possible to use self.user_shares.iter() and, after it, use self.user_shares.insert(account, new_user_balance);
                Is it a rust limitation maybe?
            */
            ext_self::balance_actualization(
                vec,
                shares.clone(),
                env::current_account_id(),
                1,
                Gas(5_000_000_000_000),
            );
        };
        shares
    }

    /// Update user balances based on the user's percentage in the Vault.
    #[payable]
    #[private]
    pub fn balance_actualization(&mut self, vec: HashMap<AccountId, u128>, shares: String) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        let new_shares_quantity = shares.parse::<u128>().unwrap();
        log!("new_shares_quantity is equal to {}", new_shares_quantity,);

        let mut total: u128 = 0;
        for (_, val) in vec.clone() {
            total = total + val
        }
        for (account, val) in vec {
            let extra_shares_for_user: u128 =
                //TODO: 0.999 to insure that it will not be given more shares than it has
                //      Find better way to handle the computation below
                ((new_shares_quantity as f64 * (val as f64 / total as f64)) * (0.999)) as u128;
            let new_user_balance = val + extra_shares_for_user;
            self.user_shares.insert(account, new_user_balance);
        }
    }

    /// Function to swap near to wnear and send it to ref.
    #[payable]
    pub fn near_to_wrap(
        &mut self,
        account_id: AccountId,
        receiver_id: AccountId,
        msg: String,
    ) {
        assert_eq!(env::predecessor_account_id(), account_id, "ERR_NOT_ALLOWED");

        let acc = self.internal_get_account(&account_id);
        let mut user_quantity: u128 = 0;
        if let Some(account) = acc {
            Some(user_quantity = account.storage_available())
        } else {
            None
        };

        assert!(user_quantity != 0, "ERROR 1: User doesnt have balance.");

        let amount: u128 = user_quantity;
        ext_wrap::near_deposit(
            CONTRACT_ID_WRAP_TESTNET.parse().unwrap(), // contract account id
            amount.to_string().parse().unwrap(),       // yocto NEAR to attach
            Gas(5_000_000_000_000),                    // gas to attach
        )
        .then(ext_wrap::ft_transfer_call(
            receiver_id,
            amount.to_string(),
            msg,
            CONTRACT_ID_WRAP_TESTNET.parse().unwrap(), // contract account id
            1,                                         // yocto NEAR to attach
            Gas(45_000_000_000_000),                   // gas to attach
        ));
    }


    /// Ref function to add liquidity in the pool.
    pub fn call_add_liquidity(
        &self,
        pool_id: u64,
        amounts: Vec<U128>,
        min_amounts: Option<Vec<U128>>,
    ) -> Promise {
        ext_exchange::add_liquidity(
            pool_id,
            amounts,
            min_amounts,
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            970000000000000000000,                // yocto NEAR to attach /////////////
            Gas(30_000_000_000_000),              // gas to attach
        )
    }

    /// Ref function to stake the lps/shares.
    pub fn call_stake(
        &self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        msg: String,
    ) -> Promise {
        ext_exchange::mft_transfer_call(
            receiver_id,
            token_id,
            amount,
            msg,
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(75_000_000_000_000),              // gas to attach
        )
    }

    /// Ref function to withdraw the rewards to exchange ref contract.
    pub fn call_withdraw_reward(
        &self,
        token_id: String,
        amount: U128,
        unregister: String,
    ) -> Promise {
        ext_farm::withdraw_reward(
            token_id,
            amount,
            unregister,
            CONTRACT_ID_FARM.parse().unwrap(), // contract account id
            1,                                 // yocto NEAR to attach
            Gas(180_000_000_000_000),          // gas to attach
        )
    }

    /// Function to return the user's deposit in the vault contract.
    pub fn get_deposits(&self, account_id: AccountId) -> HashMap<AccountId, U128> {
        let wrapped_account = self.internal_get_account(&account_id);
        if let Some(account) = wrapped_account {
            account
                .get_tokens()
                .iter()
                .map(|token| (token.clone(), U128(account.get_balance(token).unwrap())))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Responsible to add liquidity and stake.
    #[private]
    #[payable]
    pub fn callback_stake_liquidity(
        &mut self,
        account_id: AccountId,
        vault_contract: AccountId,
    ) -> Vec<U128> {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) =
                    near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens)
                {
                    is_tokens
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };
        let pool_id_to_add_liquidity = 193;
        let token_out1: AccountId = CONTRACT_ID_EHT_TESTNET.parse().unwrap();
        let token_out2: AccountId = CONTRACT_ID_DAI_TESTNET.parse().unwrap();
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1.to_string() {
                quantity_of_token1 = *val
            };
            if key.to_string() == token_out2.to_string() {
                quantity_of_token2 = *val
            };
        }
        let pool_id: u64 = 193;

        self.call_add_liquidity(
            pool_id_to_add_liquidity,
            vec![quantity_of_token2, quantity_of_token1],
            None,
        )
        .then(ext_self::call_get_pool_shares(
            pool_id.clone(),
            vault_contract,
            env::current_account_id(),
            0,
            Gas(18_000_000_000_000),
        ))
        .then(ext_self::callback_update_user_balance(
            account_id.clone(),
            env::current_account_id(),
            0,
            Gas(5_000_000_000_000),
        ))
        .then(ext_self::callback_stake(
            account_id.clone(),
            env::current_account_id(),
            0,
            Gas(90_000_000_000_000),
        ));
        let quantity_eth_dai = vec![quantity_of_token2, quantity_of_token1];
        quantity_eth_dai
    }

    /// Receives shares from auto-compound and stake it
    #[private]
    pub fn callback_stake(&mut self, account_id: AccountId) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        self.call_stake(
            CONTRACT_ID_FARM.parse().unwrap(),
            ":193".to_string(),
            U128(shares.parse::<u128>().unwrap()),
            "".to_string(),
        );
    }

    /// Change the user_balance and the vault balance of lps/shares
    #[private]
    pub fn callback_update_user_balance(&mut self, account_id: AccountId) -> String {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let vault_shares_on_pool = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares.parse::<u128>().unwrap()
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        let shares_added_to_pool = vault_shares_on_pool - self.vault_shares;
        let user_shares = self.get_user_shares(account_id.clone());

        if user_shares == None {
            self.user_shares.insert(account_id.clone(), 0);
        }

        let mut new_user_balance: u128 = 0;

        if vault_shares_on_pool > self.vault_shares {
            if let Some(x) = self.get_user_shares(account_id.clone()) {
                Some(new_user_balance = x.parse::<u128>().unwrap() + shares_added_to_pool)
            } else {
                None
            };
            self.user_shares.insert(account_id, new_user_balance);
            log!("User_shares = {}", new_user_balance);
        };
        self.vault_shares = vault_shares_on_pool;

        vault_shares_on_pool.to_string()
    }

    /// Get the reward claimed and withdraw it.
    #[payable]
    #[private]
    pub fn callback_withdraw_rewards(&mut self, token_id: String) -> U128 {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(amount) = near_sdk::serde_json::from_slice::<U128>(&tokens) {
                    ext_farm::withdraw_reward(
                        token_id,
                        amount,
                        "false".to_string(),
                        CONTRACT_ID_FARM.parse().unwrap(), // contract account id
                        1,                                 // yocto NEAR to attach
                        Gas(180_000_000_000_000),          // gas to attach
                    );
                    amount
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        //Storing reward amount
        let amount_in_u128: u128 = amount.into();

        if self.last_reward_amount.get(&"ref-finance.testnet@193#1".to_string()) == None {
            self.last_reward_amount.insert("ref-finance.testnet@193#1".to_string(), 0);
        }

        let residue: u128 = *self
            .last_reward_amount
            .get(&"ref-finance.testnet@193#1".to_string())
            .unwrap();
            
        self.last_reward_amount.insert(
            "ref-finance.testnet@193#1".to_string(),
            amount_in_u128 + residue,
        );
        log!("print: {}", (amount_in_u128 + residue));

        amount
    }

    /// Swap wnear added and stake it.
    #[payable]
    pub fn add_to_vault(&mut self, account_id: AccountId, vault_contract: AccountId) -> String {
        assert_eq!(env::predecessor_account_id(), account_id,"ERR_NOT_ALLOWED");
        let acc = self.internal_get_account(&account_id.clone());
        let mut amount_available: u128 = 0;
        if let Some(account) = acc {
            Some(amount_available = account.storage_available())
        } else {
            None
        };
        log!(
            "amount_available (sent to ref-exchange) is equal to {}",
            amount_available
        );

        let mut bool_val = true;
        if amount_available == 0 {
            bool_val = false
        };
        assert!(bool_val, "ERROR 1: User doesnt have balance.");

        let amount: u128 = amount_available;

        ///////////////Quantity of shares///////////////
        let pool_id: u64 = 193;
        self.call_get_pool_shares(pool_id.clone(), vault_contract.clone())
            .then(ext_self::callback_update_user_balance(
                account_id.clone(),
                env::current_account_id(),
                0,
                Gas(3_000_000_000_000),
        ));

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 356;                 //Id of the eth-wnear pool that will be used for swap
        let pool_id_to_swap2 = 231;                 //Id of the dai-wnear pool that will be used for swap
        let token_in1 = CONTRACT_ID_WRAP_TESTNET.parse().unwrap();
        let token_in2 = CONTRACT_ID_WRAP_TESTNET.parse().unwrap();
        let token_out1 = CONTRACT_ID_EHT_TESTNET.parse().unwrap();
        let token_out2 = CONTRACT_ID_DAI_TESTNET.parse().unwrap();
        let min_amount_out = U128(0);
        let amount_in = Some(U128(amount / 2));

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        );

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions2,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        )
        .then(ext_self::add_near_balance(account_id.clone(), amount_available, env::current_account_id(), 0, Gas(2_000_000_000_000)));

        ///////////////Adding liquidity, staking ///////////////
        self.call_get_deposits(vault_contract.clone())
        .then(ext_self::callback_stake_liquidity(
            account_id.clone(),
            vault_contract.clone(),
            env::current_account_id(),
            970000000000000000000,
            Gas(200_000_000_000_000),
        ));
        "OK!".to_string()
    }

    pub fn add_near_balance( &mut self,account_id: AccountId, amount_available: u128){
        assert!( account_id == self.owner_id || account_id == env::current_account_id(), "ERR_NOT_ALLOWED" );
        self.internal_register_account_sub(&account_id.clone(), amount_available);
    }

    pub fn sub_near_balance( &mut self,account_id: AccountId, amount_available: u128){
        assert!( account_id == self.owner_id || account_id == env::current_account_id(), "ERR_NOT_ALLOWED" );
        self.internal_register_account(&account_id.clone(), amount_available);
    }

    /// Withdraw user lps and send it to the Vault contract.
    pub fn withdraw_all(
        &mut self,
        seed_id: String,
        msg: String,
        vault_contract: AccountId,
        account_id: AccountId,
    ) {
        assert_eq!(env::predecessor_account_id(), account_id,"ERR_NOT_ALLOWED");

        let user_lps = self.user_shares.get(&account_id);
        let mut user_quantity_available_to_withdraw: u128 = 0;
        if let Some(temp) = user_lps {
            Some(user_quantity_available_to_withdraw = *temp)
        } else {
            None
        };

        self.user_shares.insert(account_id, 0);

        let pool_id: u64 = 193;
        let min_amounts: Vec<U128> = vec![U128(1000), U128(1000)];

        // Unstake shares/lps
        ext_farm::withdraw_seed(
            seed_id,
            U128(user_quantity_available_to_withdraw).clone(),
            msg,
            CONTRACT_ID_FARM.parse().unwrap(), // contract account id
            1,                                 // yocto NEAR to attach
            Gas(180_000_000_000_000),          // gas to attach 108 -> 180_000_000_000_000
        )
        .then(
            // Taking out the liquidity
            ext_exchange::remove_liquidity(
                pool_id,
                U128(user_quantity_available_to_withdraw),
                min_amounts,
                CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
                1,                                    // yocto NEAR to attach
                Gas(8_000_000_000_000),               // gas to attach
            ),
        )
        .then(ext_exchange::get_deposits(
            vault_contract.clone(),
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(9_000_000_000_000),               // gas to attach
        ))
        // Swap tokens to wrap near
        .then(ext_self::swap_to_withdraw_all(
            env::current_account_id(),
            0,
            Gas(41_500_000_000_000),
        ));
    }

    /// Second call in order to withdraw the amount deposited.
    ///
    /// Call `withdraw_all` before calling this.
    #[payable]
    pub fn withdraw_all_2(&mut self, vault_contract: AccountId, account_id: AccountId) {
        assert_eq!(env::predecessor_account_id(), account_id,"ERR_NOT_ALLOWED");

        ext_exchange::get_deposits(
            vault_contract.clone(),
            CONTRACT_ID_REF_EXC.parse().unwrap(), // contract account id
            1,                                    // yocto NEAR to attach
            Gas(10_000_000_000_000),              // gas to attach 8,5
        )
        // Withdraw wnear and send to vault
        .then(ext_self::callback_to_withdraw(
            env::current_account_id(),
            1,
            Gas(78_000_000_000_000),
        ))
        // Withdraw wnear from wrapnear
        .then(ext_self::callback_to_near_withdraw(
            account_id,
            env::current_account_id(),
            1,
            Gas(20_000_000_000_000),
        ));
    }

    #[private]
    #[payable]
    pub fn callback_to_near_withdraw(&mut self, account_id: AccountId) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(amount) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    amount
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };
        ext_wrap::near_withdraw(
            U128(amount.parse::<u128>().unwrap()),
            CONTRACT_ID_WRAP_TESTNET.parse().unwrap(),
            1,
            Gas(3_000_000_000_000),
        )
        .then(ext_self::sub_near_balance(account_id.clone(), amount.parse::<u128>().unwrap(), env::current_account_id(), 0, Gas(5_000_000_000_000)));

    }

    /// Take out wnear from ref-exchange and send it to Vault contract.
    #[private]
    #[payable]
    pub fn callback_to_withdraw(&mut self) -> U128 {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(amount) =
                    near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens)
                {
                    amount
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        let token_out = CONTRACT_ID_WRAP_TESTNET;
        let mut quantity_of_token = U128(0);

        for (key, val) in amount.iter() {
            if key.to_string() == token_out {
                quantity_of_token = *val
            };
        }

        ext_exchange::withdraw(
            CONTRACT_ID_WRAP_TESTNET.parse().unwrap(),
            quantity_of_token,
            Some(false),
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(70_000_000_000_000),
        );

        quantity_of_token
    }

    /// Swap pool tokens to wnear
    #[private]
    #[payable]
    pub fn swap_to_withdraw_all(&mut self) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) =
                    near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens)
                {
                    is_tokens
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        let token_out1 = CONTRACT_ID_EHT_TESTNET;
        let token_out2 = CONTRACT_ID_DAI_TESTNET;

        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {
                quantity_of_token1 = *val
            };
            if key.to_string() == token_out2 {
                quantity_of_token2 = *val
            };
        }

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 356;                 //Id of the eth-wnear pool that will be used for swap
        let pool_id_to_swap2 = 231;                 //Id of the dai-wnear pool that will be used for swap
        let token_out1 = CONTRACT_ID_WRAP_TESTNET.parse().unwrap();
        let token_out2 = CONTRACT_ID_WRAP_TESTNET.parse().unwrap();
        let token_in1 = CONTRACT_ID_EHT_TESTNET.parse().unwrap();
        let token_in2 = CONTRACT_ID_DAI_TESTNET.parse().unwrap();
        let min_amount_out = U128(0);
        let amount_in1 = Some(quantity_of_token1);
        let amount_in2 = Some(quantity_of_token2);

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in1,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        );

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in2,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions2,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        );
    }

    /// Swap the auto-compound rewards to ETH and DAI
    #[private]
    #[payable]
    pub fn swap_to_auto(&mut self, farm_id: String) {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(),"ERR_NOT_ALLOWED");

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) =
                    near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens)
                {
                    is_tokens
                } else {
                    env::panic_str("ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic_str("ERR_CALL_FAILED"),
        };

        let token_out3 = CONTRACT_ID_REF_TESTNET;
        let mut quantity_of_token = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out3 {
                quantity_of_token = *val
            };
        }

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 321;
        let pool_id_to_swap2 = 326;
        let token_in1 = CONTRACT_ID_REF_TESTNET.parse().unwrap();
        let token_in2 = CONTRACT_ID_REF_TESTNET.parse().unwrap();
        let token_out1 = CONTRACT_ID_EHT_TESTNET.parse().unwrap();
        let token_out2 = CONTRACT_ID_DAI_TESTNET.parse().unwrap();
        let min_amount_out = U128(0);
        let quantity_of_token: u128 = quantity_of_token.into();
        let amount_in = Some(U128(quantity_of_token / 2));

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        );

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        ext_exchange::swap(
            actions2,
            None,
            CONTRACT_ID_REF_EXC.parse().unwrap(),
            1,
            Gas(15_000_000_000_000),
        );
        //Actualization of reward amount
        self.last_reward_amount.insert(farm_id, 0);
    }
}

/// Internal methods implementation.
impl Contract {
    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic_str("E51: contract paused"),
        };
    }
}
