use std::convert::TryInto;
use std::fmt;
use std::collections::HashMap;
use std::convert::Into;
use std::collections::BTreeMap;

use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{ValidAccountId, U128, U64, Base64VecU8};
use near_sdk::{assert_one_yocto, env, log, near_bindgen, serde_json, PromiseResult, Balance, Gas,AccountId, PanicOnDefault, Promise, ext_contract,BorshStorageKey
};



use crate::account_deposit::{VAccount, Account};
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


near_sdk::setup_alloc!();


#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
    UserShares,
    VaultShares,
    Whitelist,
    AccountTokens {account_id: AccountId},
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
    Running, Paused
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
}


//Contracts addresses.
const CONTRACT_ID: &str = "exchange.ref-dev.testnet";
const CONTRACT_ID_WRAP: &str = "wrap.testnet";
const CONTRACT_ID_FARM2: &str = "farm.leopollum.testnet";

pub const NO_DEPOSIT: u128 = 0;
pub const GAS_FOR_COMPUTE_CALL: Gas = 70_000_000_000_000;
pub const GAS_FOR_COMPUTE_CALLBACK: Gas = 40_000_000_000_000;
pub const GAS_FOR_SCHEDULE_CALL: Gas = 25_000_000_000_000;
pub const GAS_FOR_SCHEDULE_CALLBACK: Gas = 5_000_000_000_000;



//Ref exchange functions that we need to call inside the vault.
#[ext_contract(ext_exchange)]
pub trait RefExchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
    );
    fn get_pool_shares(
        &mut self,
        pool_id: u64,
        account_id: AccountId,
    );
    fn metadata(&mut self);
    fn storage_deposit(
        &mut self, 
        account_id: AccountId,
    );
    fn get_deposits(
        &mut self, 
        account_id: ValidAccountId,
    );
    fn add_liquidity(
        &mut self,
        pool_id: u64,
        amounts: Vec<U128>,
        min_amounts: Option<Vec<U128>>,
    );
    fn swap(
        &mut self,
        actions:  Vec<SwapAction>,
        referral_id: Option<ValidAccountId>
    );
    fn mft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        msg: String
    );
    fn remove_liquidity(
        &mut self,
        pool_id: u64,
        shares: U128,
        min_amounts: Vec<U128>,
    );
    fn withdraw(
        &mut self,
        token_id: String,
        amount: U128,
        unregister: Option<bool>
    );
    
}


//Ref farm functions that we need to call inside the vault.
#[ext_contract(ext_farm)]
pub trait FluxusFarming {
    fn mft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        msg: String
    );
    fn claim_reward_by_seed(
        &mut self,
        seed_id: String
    );
    fn withdraw_seed(
        &mut self,
        seed_id: String,
        amount: U128,
        msg: String
    );
    fn withdraw_reward(
        &mut self,
        token_id: String,
        amount: U128,
        unregister: String
    );
    fn get_reward(
        &mut self,
        account_id: ValidAccountId,
        token_id: ValidAccountId
    );
}


//Wrap.testnet functions that we need to call inside the vault.
#[ext_contract(ext_wrap)]
pub trait Wrapnear {
    fn storage_deposit(
        &mut self
    );
    fn near_deposit(
        &mut self
    );
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: String,
        msg: String
    );
    fn near_withdraw(
        &mut self,
        amount: U128,
    );
}

//Vault functions that we need to call like a callback.
#[ext_contract(ext_self)]
pub trait VaultContract {
    fn callback_stake_liquidity(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId) ->Vec<U128>;
    fn callback_update_user_balance(&mut self, account_id: ValidAccountId) -> String;
    fn call_get_pool_shares(&mut self, pool_id: u64, account_id: AccountId) -> String;
    fn callback_withdraw_rewards(&mut self, token_id: String) -> String;
    fn swap_to_withdraw_all(&mut self);
    fn callback_to_withdraw(&mut self);
    fn callback_to_near_withdraw(&mut self,account_id: ValidAccountId);
    fn callback_stake(&mut self, account_id: ValidAccountId);
    fn schedule_callback(&mut self, #[callback] #[serializer(borsh)] task_hash: Base64VecU8,);
    fn swap_to_auto(&mut self);
    fn callback_to_balance(&mut self);
    fn stake_and_liquidity_auto(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId);
    fn balance_actualization(&mut self, vec: HashMap<AccountId, u128>, shares: String );



}

#[ext_contract(ext_reffakes)]
pub trait ExtRefFakes {
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: String,
        msg: String
    );}

#[ext_contract(ext_croncat)]
pub trait ExtCroncat {
    fn get_slot_tasks(&self, offset: Option<u64>) -> (Vec<Base64VecU8>, U128);
    fn get_tasks(
        &self,
        slot: Option<U128>,
        from_index: Option<U64>,
        limit: Option<U64>,
    ) -> Vec<Task>;
    fn get_task(&self, task_hash: String) -> Task;
    fn create_task(
        &mut self,
        contract_id: String,
        function_id: String,
        cadence: String,
        recurring: Option<bool>,
        deposit: Option<U128>,
        gas: Option<Gas>,
        arguments: Option<Vec<u8>>,
    ) -> Base64VecU8;
    fn remove_task(&mut self, task_hash: Base64VecU8);
    fn proxy_call(&mut self);
    fn get_info(
        &mut self,
    ) -> (
        bool,
        AccountId,
        U64,
        U64,
        [u64; 2],
        U128,
        U64,
        U64,
        U128,
        U128,
        U128,
        U128,
        U64,
        U64,
        U64,
        U128,
    );
}



#[near_bindgen]
impl Contract {

    //Function that initialize the contract.
    #[init]
    pub fn new(owner_id: ValidAccountId, vault_shares: u128) -> Self {
        Self {
            owner_id: owner_id.as_ref().clone(),
            user_shares: HashMap::new(),
            vault_shares,
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            state: RunningState::Running,
        }
    }

    pub fn get_user_shares(&self, account_id: ValidAccountId) -> Option<String> {
        let user_shares = self.user_shares.get(&account_id.to_string());
        if let Some(account) = user_shares {
            Some(
               account.to_string()
            )           
        } else {
            None
        }
        
    }

    
    //Extend the whitelist of tokens.
    #[payable]
    pub fn extend_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        for token in tokens {
            self.whitelisted_tokens.insert(token.as_ref());
        }
    }


    //Function that return the whitelisted tokens.
    pub fn get_whitelisted_tokens(&self) -> Vec<AccountId> {
        self.whitelisted_tokens.to_vec()
    }


    //Function that return the user`s near storage.
    pub fn get_user_storage_state(&self, account_id: ValidAccountId) -> Option<RefStorageState> {
        let acc = self.internal_get_account(account_id.as_ref());
        if let Some(account) = acc {
            Some(
                RefStorageState {
                    deposit: U128(account.near_amount),
                    usage: U128(account.storage_usage()),
                }
            )           
        } else {
            None
        }
    }


    //Call the ref get_pool_shares function.
    #[private]
    pub fn call_get_pool_shares(&self, pool_id: u64, account_id: AccountId) -> Promise {
        ext_exchange::get_pool_shares(
        pool_id,    
        account_id,    
        &CONTRACT_ID, // contract account id
        0, // yocto NEAR to attach
        10_000_000_000_000 // gas to attach
        )
    }


    //Call the ref metadata function.
    pub fn call_meta(&self) -> Promise {
        log!("Entrou na parte de teste");
        ext_exchange::metadata(
            &CONTRACT_ID, // contract account id
            0, // yocto NEAR to attach
            3_000_000_000_000 // gas to attach
        )
    }


    //Call the ref user_register function.
    pub fn call_user_register(&self, account_id: AccountId) -> Promise {
        log!("Entrei no call_user_register");
        ext_exchange::storage_deposit(
        account_id,    
        &CONTRACT_ID, // contract account id
        10000000000000000000000, // yocto NEAR to attach
        3_000_000_000_000 // gas to attach
        )
    }

    //Call the ref get_deposits function.
    fn call_get_deposits(&self, account_id: ValidAccountId) -> Promise {
        ext_exchange::get_deposits(
        account_id,    
        &CONTRACT_ID, // contract account id
        1, // yocto NEAR to attach
        15_000_000_000_000 // gas to attach
        )
    }

    #[payable]
    pub fn auto_function(&mut self) {

        //Claim & Withdraw 
        //self.withdraw_of_reward(env::current_account_id().try_into().unwrap());

    
        //Get vault's deposit
        ext_exchange::get_deposits(
            env::current_account_id().try_into().unwrap(),    
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            9_000_000_000_000 // gas to attach
        )


        //Swap ref tokens
        .then(ext_self::swap_to_auto(&env::current_account_id(), 0, 41_500_000_000_000))


        //get quantity
        .then(ext_exchange::get_deposits(
            env::current_account_id().try_into().unwrap(),    
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            8_500_000_000_000 // gas to attach
        ))


        //add liquidity and stake once again
        .then(ext_self::stake_and_liquidity_auto(//todo change something
            env::current_account_id().try_into().unwrap(),
            env::current_account_id().try_into().unwrap(), 
            &env::current_account_id(), 
            970000000000000000000, 
            200_000_000_000_000
        ));

    }


    //Function to claim and take off the reward from the farm contract to exchange contract.
    #[payable]
    pub fn withdraw_of_reward(&mut self) {

        let token_id = "ref.fakes.testnet".to_string();
        let seed_id = "exchange.ref-dev.testnet@193".to_string();

        self.call_claim(seed_id.clone())  
        .then(self.call_get_reward(env::current_account_id().try_into().unwrap(), "ref.fakes.testnet".try_into().unwrap()))
        .then(ext_self::callback_withdraw_rewards(token_id, &env::current_account_id(), 1, 190_000_000_000_000))//passar exatamente 190
        .then(ext_reffakes::ft_transfer_call( 
            "exchange.ref-dev.testnet".to_string(),//receiver_id,
            "5000000000000000000".to_string(),//5 refs
            "".to_string(),
            &"ref.fakes.testnet", // contract account id
            1, // yocto NEAR to attach
            50_000_000_000_000 // gas to attach   
        ));
    
    }

        

    //Responsible to add liquidity and stake 
    #[private]
    #[payable]
    pub fn stake_and_liquidity_auto(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId) {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) = near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens) {
                    is_tokens
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };   
        let pool_id_to_add_liquidity = 193;
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {quantity_of_token1 = *val};
            if key.to_string() == token_out2 {quantity_of_token2 = *val};
        }
       
        let pool_id: u64 = 193;
        let seed_id = "exchange.ref-dev.testnet@193".to_string();


        //Adding liquidity
        self.call_add_liquidity(pool_id_to_add_liquidity, vec![quantity_of_token2, quantity_of_token1], None)
        
        //Getting the shares
        .then(ext_exchange::get_pool_shares(
              pool_id,    
              account_id.clone().try_into().unwrap(),    
              &CONTRACT_ID, // contract account id
              0, // yocto NEAR to attach
              10_000_000_000_000 // gas to attach
        ))
        .then(ext_self::callback_to_balance(&env::current_account_id(), 0, 15_000_000_000_000))
        //todo: verificar a passagem do parâmetro shares daqui pra la
        .then(ext_self::callback_stake(account_id.clone(), &env::current_account_id(), 0, 90_000_000_000_000));

    }

    #[private]
    pub fn callback_to_balance(&mut self) -> String {
        
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };  


        //If new_shares_quantity > 0: 
        if shares.parse::<u128>().unwrap() > 0 {
            log!("entra no if");
            let mut vec:HashMap<AccountId, u128> = HashMap::new();

            for (account, val) in self.user_shares.iter()
            {
                vec.insert(account.to_string(), *val);
            }

            ext_self::balance_actualization(vec, shares.clone(), &env::current_account_id(), 1, 5_000_000_000_000);
            
        };
   
        shares
    }

    #[payable]
    #[private]
    pub fn balance_actualization(&mut self, vec: HashMap<AccountId, u128>,shares: String ) {

        let new_shares_quantity = shares.parse::<u128>().unwrap();
        log!("new_shares_quantity é =  {} ", new_shares_quantity,);

        let mut total: u128 = 0;
        for (account, val) in vec.clone() {total = total +val}
        for (account, val) in vec
        {
            log!("total = {}", total);
            log!("val/total = {}", val/total);

            let y = new_shares_quantity * (val/total);
            let x = val + y;
            log!("{} tinha {} mas estou somando {} e agora terá = {}", account, val, y, x );
            self.user_shares.insert(account, x); 
            
        }

    }


    #[payable]
    //Function to change near in wrap near and send it to ref.
    pub fn near_to_wrap(&mut self, account_id: ValidAccountId, receiver_id: AccountId, amount: String, msg: String) {
        
        log!("Entrei no near_to_wrap");
        let acc = self.internal_get_account(account_id.as_ref());
        let mut user_quantity: u128 = 0; 
        if let Some(account) = acc {
            Some(
                user_quantity = account.storage_available()
            )     
        } else {
            None
        };

        let mut bool_val = true;
        let quantity =  amount.parse::<u128>().unwrap();
        if user_quantity < quantity {bool_val = false};
        assert!(bool_val, "ERROR 1: User doesnt have balance.");

        //self.internal_register_account_sub(&account_id.to_string(), user_quantity);//quantity);///////////////////todo
        let amount:u128 = user_quantity;// amount.parse::<u128>().unwrap();
        log!("amount que vai pra ref é = {}",amount);
        
        /*
        ext_wrap::storage_deposit(
            &CONTRACT_ID_WRAP, // contract account id
            1250000000000000000000, // yocto NEAR to attach
            35_000_000_000_000 // gas to attach
        )
        .then(*/
            ext_wrap::near_deposit(
                &CONTRACT_ID_WRAP, // contract account id
                amount.to_string().parse().unwrap(), // yocto NEAR to attach
                3_000_000_000_000 // gas to attach
            )
        //)
        .then(
            ext_wrap::ft_transfer_call(
                receiver_id,//receiver_id,
                amount.to_string(),
                msg,
                &CONTRACT_ID_WRAP, // contract account id
                1, // yocto NEAR to attach
                35_000_000_000_000 // gas to attach                
            )
        );

    }

    //Ref function to swap tokens.
    pub fn call_swap(&self, actions: Vec<SwapAction>, referral_id: Option<ValidAccountId> ) -> Promise {
        ext_exchange::swap(
        actions,   
        referral_id,
        &CONTRACT_ID, // contract account id
        1, // yocto NEAR to attach /////////////
        15_000_000_000_000 // gas to attach
        )
    }


    //Ref function to add liquidity in a pool.
    pub fn call_add_liquidity(&self, pool_id: u64, amounts: Vec<U128>, min_amounts: Option<Vec<U128>>) -> Promise {
        ext_exchange::add_liquidity(
        pool_id,
        amounts,
        min_amounts,   
        &CONTRACT_ID, // contract account id
        970000000000000000000, // yocto NEAR to attach /////////////
        30_000_000_000_000 // gas to attach
        )
    }

    
    //Ref function to stake the lps/shares.
    pub fn call_stake(&self, receiver_id: AccountId, token_id: String, amount: U128, msg: String) -> Promise {
        ext_exchange::mft_transfer_call(
            receiver_id,
            token_id,
            amount,
            msg,
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            75_000_000_000_000 // gas to attach
        )
    }


    //Ref function to claim the vault rewards.
    pub fn call_claim(&self, seed_id: String) -> Promise {
        log!("Entrei no call_claim");
        ext_farm::claim_reward_by_seed(
            seed_id,
            &CONTRACT_ID_FARM2, // contract account id
            0, // yocto NEAR to attach
            30_000_000_000_000 // gas to attach
        )
    }

    
    //Ref function to unstake lps/shares.
    pub fn call_unstake(&self, seed_id: String, amount: U128, msg: String) -> Promise {
        log!("Entrei no call_unstake");
        ext_farm::withdraw_seed(
            seed_id,
            amount,
            msg,
            &CONTRACT_ID_FARM2, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        )
    }


    //Ref function to withdraw the rewards to exchange ref contract.
    pub fn call_withdraw_reward(&self, token_id: String, amount: U128, unregister: String) -> Promise {
        //Registro de usuário
        log!("Entrei no call_withdraw_reward");
        ext_farm::withdraw_reward(
            token_id,
            amount,
            unregister,
            &CONTRACT_ID_FARM2, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        )
    }


    //Ref function to return the amount of rewards in the farm contract.
    pub fn call_get_reward(&self, account_id: ValidAccountId, token_id: ValidAccountId) -> Promise {
        //Registro de usuário
        log!("Entrei no call_get_reward");
        ext_farm::get_reward(
            account_id,
            token_id,
            &CONTRACT_ID_FARM2, // contract account id
            1, // yocto NEAR to attach
            3_000_000_000_000 // gas to attach
        )
    }


    //Function to return the user's deposit in the vault contract.
    pub fn get_deposits(&self, account_id: ValidAccountId) -> HashMap<AccountId, U128> /*StorageBalance*/ {

        let wrapped_account = self.internal_get_account(account_id.as_ref());
        if let Some(account) = wrapped_account {
            account.get_tokens()
                .iter()
                .map(|token| (token.clone(), U128(account.get_balance(token).unwrap())))
                .collect()
        } else {
            HashMap::new()
        }
    }



    #[payable]
    pub fn take_the_reward(&mut self) {

        let token_id = "ref.fakes.testnet".to_string();
        let seed_id = "exchange.ref-dev.testnet@193".to_string();

        self.call_claim(seed_id.clone())  
        //.then(self.call_get_reward(env::current_account_id().try_into().unwrap(), "ref.fakes.testnet".try_into().unwrap()))
        ;
        //.then(ext_self::callback_withdraw_rewards(token_id, &env::current_account_id(), 1, 190_000_000_000_000));//passar exatamente 190
    }

    #[payable]
    pub fn test_swap(&mut self) {

        let pool_id_to_swap1 = 83;
        let token_in1 = "wrap.testnet".to_string();
        let token_out1 = "eth.fakes.testnet".to_string();
        let min_amount_out = U128(0);
        let amount_in = Some(U128(100000000));

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,//Todo
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions, None);
        ext_exchange::swap( actions, None, &CONTRACT_ID, 1, 15_000_000_000_000);
        //near call exchange.ref-dev.testnet swap '{"actions": [{"pool_id": "83","token_in":"wrap.testnet","amount_in":"99","token_out": "eth.fakes.testnet","min_amount_out: "0"}],"referral_id": null}' --accountId dev-1643376891746-20538337921288  --gas 300000000000000 --deposit 0.000000000000000000000001

    }

    //Responsible to add liquidity and stake 
    #[private]
    #[payable]
    pub fn callback_stake_liquidity(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId) -> Vec<U128> {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) = near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens) {
                    is_tokens
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };   
        let pool_id_to_add_liquidity = 193;
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {quantity_of_token1 = *val};
            if key.to_string() == token_out2 {quantity_of_token2 = *val};
        }
       
        let pool_id: u64 = 193;
        let seed_id = "exchange.ref-dev.testnet@193".to_string();


        self.call_add_liquidity(pool_id_to_add_liquidity, vec![quantity_of_token2, quantity_of_token1], None)
        .then(ext_self::call_get_pool_shares(pool_id.clone(), vault_contract.clone().to_string(),&env::current_account_id(), 0, 18_000_000_000_000))
        .then(ext_self::callback_update_user_balance(account_id.clone(), &env::current_account_id(), 0, 5_000_000_000_000))
        .then(ext_self::callback_stake(account_id.clone(), &env::current_account_id(), 0, 90_000_000_000_000));
        //self.user_shares.get(&vault_contract.to_string()
        let x = vec![quantity_of_token2, quantity_of_token1];
        x

    }

    #[private]
    pub fn callback_stake(&mut self, account_id: ValidAccountId ) {
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };  

        self.call_stake(
            CONTRACT_ID_FARM2.to_string(), 
            ":193".to_string(), 
            U128(shares.parse::<u128>().unwrap()), 
            "".to_string()
        );
    
    }    
   


    //Change the user_balance and the vault balance of lps/shares
    #[private]
    pub fn callback_update_user_balance(&mut self, account_id: ValidAccountId ) -> String {
        
    assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let vault_shares_on_pool = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    shares.parse::<u128>().unwrap()
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            }
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };

        log!("doing vault_shares_on_pool - vault_shares == {} - {}", vault_shares_on_pool.clone(), self.vault_shares.clone());

        let shares_added_to_pool = vault_shares_on_pool - self.vault_shares;
        // let user_shares = self.user_shares.get(&account_id.to_string());
        let user_shares = self.get_user_shares(account_id.clone());

        log!("user_shares at the moment == {:?}", user_shares.clone());
        if user_shares == None {
            self.user_shares.insert(account_id.to_string(), 0);
         }

        let mut new_user_balance: u128 = 0;

        if vault_shares_on_pool > self.vault_shares {
            if let Some(x) = self.get_user_shares(account_id.clone()) {
                log!("doing user_shares + shares_added_to_pool == {} + {}", x.clone(), shares_added_to_pool.clone());
                Some(new_user_balance = x.parse::<u128>().unwrap() + shares_added_to_pool)
            } else {
                None
            };
            self.user_shares.insert(account_id.to_string(), new_user_balance);
            log!("User_shares= {}", new_user_balance);
        };
        self.vault_shares = vault_shares_on_pool;

        vault_shares_on_pool.to_string()
    }

    //Get the reward claimed and withdraw it.
    #[payable]
    #[private]
    pub fn callback_withdraw_rewards(&mut self, token_id: String) -> U128 {
        
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<U128>(&tokens) {
                    ext_farm::withdraw_reward(
                        token_id,
                        shares,
                        "false".to_string(),
                        &CONTRACT_ID_FARM2, // contract account id
                        1, // yocto NEAR to attach
                        180_000_000_000_000 // gas to attach
                    );
                    shares
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };  
        
        shares
    }


    //Main vault function
    #[payable]
    pub fn add_to_vault(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId /*, amount: String, msg: String*/) -> String  {

        let acc = self.internal_get_account(account_id.as_ref());
        let mut amount_available: u128 = 0; 
        if let Some(account) = acc {
            Some(
                amount_available = account.storage_available()
            )     
        } else {
            None
        };

        //Log user full wrap near amount 
        //let x:u128 = 50000000000000000000000;
        //log!("amount_available - x é igual a: {}", amount_available - x);
        log!("amount_available (o que o usuário teoricamente mandou pra ref) é igual a: {}", amount_available);

        let mut bool_val = true;
        if amount_available == 0 {bool_val = false};
        assert!(bool_val, "ERROR 1: User doesnt have balance.");

        let amount:u128 = amount_available ;// - 144036519938737445411117;

        ///////////////Quantity of shares///////////////
        let pool_id: u64 = 193;
        self.call_get_pool_shares(pool_id.clone(), vault_contract.clone().to_string())
        .then(ext_self::callback_update_user_balance(account_id.clone(), &env::current_account_id(), 0, 3_000_000_000_000));

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 83;
        let pool_id_to_swap2 = 84;
        let token_in1 = "wrap.testnet".to_string();
        let token_in2 = "wrap.testnet".to_string();
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let min_amount_out = U128(0);
        let amount_in = Some(U128(amount/2));

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,//Todo
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions, None);
        ext_exchange::swap( actions, None, &CONTRACT_ID, 1, 15_000_000_000_000);

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,//Todo
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions2, None);
        ext_exchange::swap( actions2, None, &CONTRACT_ID, 1, 15_000_000_000_000);



        self.internal_register_account_sub(&account_id.to_string(), amount_available);//quantity);///////////////////

        
        ///////////////Adding liquidity, staking ///////////////
        self.call_get_deposits(vault_contract.clone())
        .then(ext_self::callback_stake_liquidity(account_id.clone(),vault_contract.clone(), &env::current_account_id(), 970000000000000000000, 200_000_000_000_000));//Passar 70 sem o stake rola.
    
     
        "OK!".to_string()
        
    }
    
    //Withdraw user lps and sending it to vault contract
    pub fn withdraw_all(&mut self, seed_id: String, /*amount: String,*/ msg: String, vault_contract: ValidAccountId, account_id: ValidAccountId) /*-> Promise*/ {

        
        let user_lps = self.user_shares.get(&account_id.to_string());
        let mut user_quantity_available_to_withdraw: u128 = 0; 
        if let Some(temp) = user_lps {
            Some(
                user_quantity_available_to_withdraw = *temp
            )     
        } else {
            None
        };

        /*
        let mut k = true;
        let quantity = amount.parse::<u128>().unwrap();
        if user_quantity_available_to_withdraw < quantity {k = false};
        assert!(k, "ERROR 2: User doesn't have lps for this.");

        let value = user_quantity_available_to_withdraw - quantity;
        self.user_shares.insert(&account_id.to_string(), &value);
        */

        
        
        self.user_shares.insert(account_id.to_string(), 0);


        let pool_id: u64 = 193;
        let min_amounts:Vec<U128> = vec![U128(1000),U128(1000)];
        //let amount: u128 = amount.parse().unwrap();

        //Unstake shares/lps
        ext_farm::withdraw_seed(
            seed_id,
            U128(user_quantity_available_to_withdraw).clone(),//quantity todo
            msg,
            &CONTRACT_ID_FARM2, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach 108 -> 180_000_000_000_000
        )
        
        .then(
        //Taking off the liquidity
        ext_exchange::remove_liquidity(
            pool_id,
            U128(user_quantity_available_to_withdraw),
            min_amounts,
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            8_000_000_000_000 // gas to attach
        ))

        //.then(self.call_get_deposits(vault_contract.clone()))
        .then(ext_exchange::get_deposits(
            vault_contract.clone(),    
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            9_000_000_000_000 // gas to attach
        ))


        //Swap tokens to wrap near
        .then(ext_self::swap_to_withdraw_all(&env::current_account_id(), 0, 41_500_000_000_000));

    } 

    #[payable]
    pub fn withdraw_all_2(&mut self, vault_contract: ValidAccountId, account_id: ValidAccountId) {
        
        ext_exchange::get_deposits(
            vault_contract.clone(),    
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            10_000_000_000_000 // gas to attach 8,5
        )

        //Withdraw wrap near and send to vault
        .then(ext_self::callback_to_withdraw(&env::current_account_id(), 1, 78_000_000_000_000))//76,5
        
        //Switching wrap near into near
        .then(ext_self::callback_to_near_withdraw(account_id, &env::current_account_id(), 1, 13_000_000_000_000)); //11

        
    }




    #[private]
    #[payable]
    pub fn callback_to_near_withdraw(&mut self,account_id: ValidAccountId) {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(amount) = near_sdk::serde_json::from_slice::<String>(&tokens) {
                    amount
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };  
        ext_wrap::near_withdraw(U128(amount.parse::<u128>().unwrap()), &CONTRACT_ID_WRAP, 1, 3_000_000_000_000);
        log!("amount no callback_to_near_withdraw {}",amount);
        self.internal_register_account_string(&account_id.to_string(), amount.clone());//todo

    }


    #[private]
    #[payable]
    pub fn callback_to_withdraw(&mut self) -> U128 {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let amount = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(amount) = near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens) {
                    amount
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };   

        let token_out3 = "wrap.testnet".to_string();
        let mut quantity_of_token3 = U128(0);

        for (key, val) in amount.iter() {
            if key.to_string() == token_out3 {quantity_of_token3 = *val};

        }

        ext_exchange::withdraw("wrap.testnet".to_string(), quantity_of_token3, Some(false),  &CONTRACT_ID, 1, 70_000_000_000_000);

        quantity_of_token3
    }

    #[private]
    #[payable]
    pub fn swap_to_withdraw_all(&mut self) /*-> U128*/ {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) = near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens) {
                    is_tokens
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };   

        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        //let token_out3 = "wrap.testnet".to_string();

        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);
        //let mut quantity_of_token3 = U128(0);


        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {quantity_of_token1 = *val};
            if key.to_string() == token_out2 {quantity_of_token2 = *val};
            //if key.to_string() == token_out3 {quantity_of_token3 = *val};

        }

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 83;
        let pool_id_to_swap2 = 84;
        let token_out1 = "wrap.testnet".to_string();
        let token_out2 = "wrap.testnet".to_string();
        let token_in1 = "eth.fakes.testnet".to_string();
        let token_in2 = "dai.fakes.testnet".to_string();
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
        //self.call_swap(actions, None);
        ext_exchange::swap( actions, None, &CONTRACT_ID, 1, 15_000_000_000_000);

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in2,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions2, None);
        ext_exchange::swap( actions2, None, &CONTRACT_ID, 1, 15_000_000_000_000);

        //quantity_of_token3
    }


    #[private]
    #[payable]
    pub fn swap_to_auto(&mut self) {

        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let is_tokens = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(is_tokens) = near_sdk::serde_json::from_slice::<HashMap<AccountId, U128>>(&tokens) {
                    is_tokens
                } else {
                    env::panic(b"ERR_WRONG_VAL_RECEIVED")
                }
            },
            PromiseResult::Failed => env::panic(b"ERR_CALL_FAILED"),
        };   

  
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let token_out3 = "ref.fakes.testnet".to_string();

        let mut quantity_of_token = U128(0);
  

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out3 {quantity_of_token = *val};

        }


        let pool_id: u64 = 193;

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 321;
        let pool_id_to_swap2 = 326;
        let token_in1 = "ref.fakes.testnet".to_string();
        let token_in2 = "ref.fakes.testnet".to_string();
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let min_amount_out = U128(0);
        let quantity_of_token: u128 = quantity_of_token.into();
        let amount_in = Some(U128(quantity_of_token/2));

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,//Todo
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions, None);
        ext_exchange::swap( actions, None, &CONTRACT_ID, 1, 15_000_000_000_000);

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,//Todo
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        //self.call_swap(actions2, None);
        ext_exchange::swap( actions2, None, &CONTRACT_ID, 1, 15_000_000_000_000);

    }


}



/// Internal methods implementation.
impl Contract {

    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic("E51: contract paused".as_bytes()),
        };
    }

}
