use std::convert::TryInto;
use std::fmt;
use std::collections::HashMap;

use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{assert_one_yocto, env, log, near_bindgen, PromiseResult, Balance, AccountId, PanicOnDefault, Promise, ext_contract,BorshStorageKey
};



use crate::account_deposit::{VAccount, Account};
mod account_deposit;
//mod owner;
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
    //Pools,
    Accounts,
    //Shares { pool_id: u32 },
    UserShares,
    VaultShares,
    Whitelist,
    //Guardian,
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
    //exchange_fee: u32,
    user_shares: LookupMap<AccountId, u128>,
    vault_shares: u128,
    //pools: Vector<Pool>,
    accounts: LookupMap<AccountId, VAccount>,
    whitelisted_tokens: UnorderedSet<AccountId>,
    //guardians: UnorderedSet<AccountId>,
    state: RunningState,
}


const CONTRACT_ID: &str = "exchange.ref-dev.testnet";
const CONTRACT_ID_WRAP: &str = "wrap.testnet";
const CONTRACT_ID_FARM: &str = "farm110.ref-dev.testnet";
//&env::current_account_id();

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
    );
    
}


#[ext_contract(ext_farm)]
pub trait RefFarming {
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
}


#[ext_contract(ext_self)]
pub trait VaultContract {
    fn callback_get_deposits(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId) ->Vec<U128>;
    fn callback_get_pool_shares(&mut self, account_id: ValidAccountId, txt: String) -> String;
    fn call_get_pool_shares(&mut self, pool_id: u64, account_id: AccountId) -> String;
    fn callback_get_reward(&mut self, token_id: String) -> String;
    fn swap_to_withdraw_all(&mut self);
    fn near_quantity_to_withdraw(&mut self);
}



#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: ValidAccountId, vault_shares: u128/*, exchange_fee: u32, referral_fee: u32*/) -> Self {
        Self {
            owner_id: owner_id.as_ref().clone(),
            //exchange_fee,
            user_shares: LookupMap::new(StorageKey::UserShares),
            vault_shares,
            //pools: Vector::new(StorageKey::Pools),
            accounts: LookupMap::new(StorageKey::Accounts),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            //guardians: UnorderedSet::new(StorageKey::Guardian),
            state: RunningState::Running,
        }
    }

    
    #[payable]
    pub fn extend_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        for token in tokens {
            self.whitelisted_tokens.insert(token.as_ref());
        }
    }

    /// Get contract level whitelisted tokens.
    pub fn get_whitelisted_tokens(&self) -> Vec<AccountId> {
        self.whitelisted_tokens.to_vec()
    }

    /// Get user's storage deposit and needed in the account of current version
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

    #[private]
    pub fn call_get_pool_shares(&self, pool_id: u64, account_id: AccountId) -> Promise {
        //Registro de usuário
        log!("Entrei no call_get_pool_shares");
        ext_exchange::get_pool_shares(
        pool_id,    
        account_id,    
        &CONTRACT_ID, // contract account id
        0, // yocto NEAR to attach
        10_000_000_000_000 // gas to attach
        )
    }



    pub fn call_meta(&self) -> Promise {
        log!("Entrou na parte de teste");
        ext_exchange::metadata(
            &CONTRACT_ID, // contract account id
            0, // yocto NEAR to attach
            3_000_000_000_000 // gas to attach
        )
    }
    pub fn call_user_register(&self, account_id: AccountId) -> Promise {
        //Registro de usuário
        log!("Entrei no call_user_register");
        ext_exchange::storage_deposit(
        account_id,    
        &CONTRACT_ID, // contract account id
        10000000000000000000000, // yocto NEAR to attach
        3_000_000_000_000 // gas to attach
        )
    }

    fn call_get_deposits(&self, account_id: ValidAccountId) -> Promise {
        //Registro de usuário
        log!("Entrei no call_get_deposits");
        ext_exchange::get_deposits(
        account_id,    
        &CONTRACT_ID, // contract account id
        10000000000000000000000, // yocto NEAR to attach
        30_000_000_000_000 // gas to attach
        )
    }

    #[payable]
    //Para trocar near em wnear
    pub fn near_to_wrap(&mut self, account_id: ValidAccountId, receiver_id: AccountId, amount: String, msg: String) {
        //Registro de usuário
        log!("Entrei no near_to_wrap");
        let acc = self.internal_get_account(account_id.as_ref());
        let mut x: u128 = 0; 
        if let Some(account) = acc {
            Some(
                x = account.storage_available()
            )     
        } else {
            None
        };

        log!("x é igual a: {}", x);

        let mut k = true;
        log!("{} - {}", x,amount.parse::<u128>().unwrap() );
        //let y: i128 = x - ;
        if x < amount.parse::<u128>().unwrap() {k = false};
        assert!(k, "ERRO 1: User doesnt have balance.");

        let amount:u128 = x;

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

    pub fn call_swap(&self, actions: Vec<SwapAction>, referral_id: Option<ValidAccountId> ) -> Promise {
        //Registro de usuário
        log!("Entrei no call_swap");
        ext_exchange::swap(
        actions,   
        referral_id,
        &CONTRACT_ID, // contract account id
        10000000000000000000000, // yocto NEAR to attach
        10_000_000_000_000 // gas to attach
        )
    }




    pub fn call_add_liquidity(&self, pool_id: u64, amounts: Vec<U128>, min_amounts: Option<Vec<U128>>) -> Promise {
        //Registro de usuário
        log!("Entrei no call_add_liquidity");
        ext_exchange::add_liquidity(
        pool_id,
        amounts,
        min_amounts,   
        &CONTRACT_ID, // contract account id
        970000000000000000000, // yocto NEAR to attach
        30_000_000_000_000 // gas to attach
        )
    }

    

    pub fn call_stake(&self, receiver_id: AccountId, token_id: String, amount: U128, msg: String) -> Promise {
        //Registro de usuário
        log!("Entrei no call_stake");
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


    pub fn call_claim(&self, seed_id: String) -> Promise {
        //Registro de usuário
        log!("Entrei no call_claim");
        ext_farm::claim_reward_by_seed(
            seed_id,
            &CONTRACT_ID_FARM, // contract account id
            0, // yocto NEAR to attach
            30_000_000_000_000 // gas to attach
        )
    }

    pub fn call_unstake(&self, seed_id: String, amount: U128, msg: String) -> Promise {
        //Registro de usuário
        log!("Entrei no call_unstake");
        ext_farm::withdraw_seed(
            seed_id,
            amount,
            msg,
            &CONTRACT_ID_FARM, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        )
    }

    pub fn call_withdraw_reward(&self, token_id: String, amount: U128, unregister: String) -> Promise {
        //Registro de usuário
        log!("Entrei no call_withdraw_reward");
        ext_farm::withdraw_reward(
            token_id,
            amount,
            unregister,
            &CONTRACT_ID_FARM, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        )
    }

    pub fn call_get_reward(&self, account_id: ValidAccountId, token_id: ValidAccountId) -> Promise {
        //Registro de usuário
        log!("Entrei no call_get_reward");
        ext_farm::get_reward(
            account_id,
            token_id,
            &CONTRACT_ID_FARM, // contract account id
            1, // yocto NEAR to attach
            3_000_000_000_000 // gas to attach
        )
    }


    fn get_deposits(&self, account_id: ValidAccountId) -> HashMap<AccountId, U128> /*StorageBalance*/ {

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

/*
    #[payable]
    pub fn near_to_Wnear(&mut self) {

        let amount: u128 = 500000000000000000000000;//= x/2;1250000000000000000000
        log!("Esse é o amount do user que vai ser mandado para a ref: {}", amount);
        self.near_to_wrap(CONTRACT_ID.to_string(), amount.to_string(), "".to_string());
        
    }*/

    #[payable]
    pub fn withdraw_of_reward(&mut self,vault_contract: ValidAccountId) {

        let token_id = "ref.fakes.testnet".to_string();
        self.call_get_reward(vault_contract.clone(), "ref.fakes.testnet".try_into().unwrap())
        .then(ext_self::callback_get_reward(token_id, &env::current_account_id(), 1, 190_000_000_000_000));//passar exatamente 190
    }


    #[private]
    #[payable]
    pub fn callback_get_deposits(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId) -> Vec<U128> {
        /* */
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
        let token_id = "ref.fakes.testnet".to_string();
        let amount_to_withdraw = U128(13586875027751185025);


        self.call_add_liquidity(pool_id_to_add_liquidity, vec![quantity_of_token2, quantity_of_token1], None)
        .then(ext_self::call_get_pool_shares(pool_id.clone(), vault_contract.clone().to_string(),&env::current_account_id(), 0, 18_000_000_000_000))
        .then(ext_self::callback_get_pool_shares(account_id.clone(),"segunda vez".to_string(), &env::current_account_id(), 0, 5_000_000_000_000))
        .then(self.call_stake(CONTRACT_ID_FARM.to_string(), ":193".to_string(), U128(self.user_shares.get(&account_id.to_string()).expect("sla oq")), "".to_string()))
        .then(self.call_claim(seed_id.clone()))       
        .then(self.call_get_reward(vault_contract.clone(), "ref.fakes.testnet".try_into().unwrap()))
        ;
        //.then(ext_self::callback_get_reward(token_id, &env::current_account_id(), 1, 250_000_000_000_000));

        let x = vec![quantity_of_token2, quantity_of_token1];
        x

    }

    #[private]
    pub fn callback_get_pool_shares(&mut self, account_id: ValidAccountId, txt: String) -> String {
        
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


        log!("Fazendo {} - {} ", shares.parse::<u128>().unwrap(), self.vault_shares);
        let x = shares.parse::<u128>().unwrap() - self.vault_shares;
        let y = self.user_shares.get(&account_id.to_string());
        let mut k: u128 = 0;

        if shares.parse::<u128>().unwrap() > self.vault_shares {
            if let Some(yy) = y {
                Some(
                    k = yy + x
                )     
            } else {
                None
            };
            self.user_shares.insert(&account_id.to_string(), &k);
            log!("{}",k)
        };
        self.vault_shares = shares.parse::<u128>().unwrap();

        shares
    }


    #[payable]
    #[private]
    pub fn callback_get_reward(&mut self, token_id: String) -> U128 {
        
        assert_eq!(env::promise_results_count(), 1, "ERR_TOO_MANY_RESULTS");
        let shares = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(tokens) => {
                if let Ok(shares) = near_sdk::serde_json::from_slice::<U128>(&tokens) {
                    ext_farm::withdraw_reward(
                        token_id,
                        shares,
                        "false".to_string(),
                        &CONTRACT_ID_FARM, // contract account id
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

        log!("PASSANDO POR AQUI NO CALLBACK ");
        
    
        shares
    }

    //Main vault function
    #[payable]
    pub fn add_to_vault(&mut self, account_id: ValidAccountId, vault_contract: ValidAccountId /*, amount: String, msg: String*/) -> String  {

        ///////////////Sending wrap near to ref//////////////////
        //Getting user's near deposits.
 /**/       
        let acc = self.internal_get_account(account_id.as_ref());
        let mut x: u128 = 0; 
        if let Some(account) = acc {
            Some(
                x = account.storage_available()
            )     
        } else {
            None
        };

        log!("x é igual a: {}", x);

        let mut k = true;
        if x == 0 {k = false};
        assert!(k, "ERRO 1: User doesnt have balance.");

        //let amount: u128 = 500000000000000000000000;//= x/2;1250000000000000000000
        let amount:u128 = x;
        log!("Esse é o amount do user que vai ser mandado para a ref: {}", amount);

        //self.near_to_wrap(CONTRACT_ID.to_string(), amount.to_string(), "".to_string());
        

        ///////////////Quantity of shares///////////////
        let pool_id: u64 = 193;
        let txt: String = "primeira vez".to_string();
        self.call_get_pool_shares(pool_id.clone(), vault_contract.clone().to_string())
        .then(ext_self::callback_get_pool_shares(account_id.clone(),txt, &env::current_account_id(), 0, 3_000_000_000_000));

        ///////////////Swapping Near to others///////////////
        let pool_id_to_swap1 = 83;
        let pool_id_to_swap2 = 84;
        let token_in1 = "wrap.testnet".to_string();
        let token_in2 = "wrap.testnet".to_string();
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let min_amount_out = U128(0);
        let amount_in = Some(U128(amount/2));

        log!("Fazendo swap");

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,//Todo
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        self.call_swap(actions, None);

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,//Todo
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in,
            min_amount_out: min_amount_out,
        }];
        self.call_swap(actions2, None);

        
        ///////////////Adding liquidity, staking and claiming rewards///////////////
       
        self.call_get_deposits(vault_contract.clone())
        .then(ext_self::callback_get_deposits(account_id.clone(),vault_contract.clone(), &env::current_account_id(), 970000000000000000000, 200_000_000_000_000));//Passar 70 sem o stake rola.
        
     
 
        ///////////////Updating the user balance of tokens, near and lp///////////////
        //subtracting the near used of users account
        self.internal_register_account_sub(&account_id.to_string(), amount);


        "OK!".to_string()
        
    }



 
    pub fn withdraw_all(&self, seed_id: String, amount: U128, msg: String, vault_contract: ValidAccountId) /*-> Promise*/ {

        //Registro de usuário
        log!("Entrei no withdraw_all");
        //let vault_contract
        let pool_id: u64 = 193;
        let min_amounts:Vec<U128> = vec![U128(1000),U128(1000)];
        //Fazendo unstake do lp
        ext_farm::withdraw_seed(
            seed_id,
            amount.clone(),
            msg,
            &CONTRACT_ID_FARM, // contract account id
            1, // yocto NEAR to attach
            110_000_000_000_000 // gas to attach
        )
        .then(
        //Taking off the liquidity
        ext_exchange::remove_liquidity(
            pool_id,
            amount,
            min_amounts,
            &CONTRACT_ID, // contract account id
            1, // yocto NEAR to attach
            110_000_000_000_000 // gas to attach
        ))
        //.then(self.call_get_deposits(vault_contract.clone()))
        //.then(ext_self::swap_to_withdraw_all(&env::current_account_id(), 0, 60_000_000_000_000))
        ;//.then(self.call_get_deposits(vault_contract.clone()))
        //.then(ext_self::near_quantity_to_withdraw(&env::current_account_id(), 1, 10_000_000_000_000));


    } 

    #[private]
    #[payable]
    pub fn swap_to_withdraw_all(&mut self) /*-> Promise*/ {

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
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {quantity_of_token1 = *val};
            if key.to_string() == token_out2 {quantity_of_token2 = *val};
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

        log!("Fazendo swap");

        let actions = vec![SwapAction {
            pool_id: pool_id_to_swap1,//Todo
            token_in: token_in1,
            token_out: token_out1,
            amount_in: amount_in1,
            min_amount_out: min_amount_out,
        }];
        self.call_swap(actions, None);

        let actions2 = vec![SwapAction {
            pool_id: pool_id_to_swap2,//Todo
            token_in: token_in2,
            token_out: token_out2,
            amount_in: amount_in2,
            min_amount_out: min_amount_out,
        }];
        self.call_swap(actions2, None);
    }

    #[private]
    #[payable]
    pub fn near_quantity_to_withdraw(&mut self) /*Promise*/ {

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

        let token_out1 = "wrap.testnet".to_string();
        let mut quantity_of_token1 = U128(0);

        for (key, val) in is_tokens.iter() {
            if key.to_string() == token_out1 {quantity_of_token1 = *val};
        }

        ext_exchange::withdraw("ref.fakes.testnet".to_string(), quantity_of_token1, &CONTRACT_ID, 1, 10_000_000_000_000);

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
