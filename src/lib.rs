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
use near_sdk::{assert_one_yocto, env, log, near_bindgen,Balance, AccountId, PanicOnDefault, Promise, ext_contract,BorshStorageKey
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
    //referral_fee: u32,
    //pools: Vector<Pool>,
    accounts: LookupMap<AccountId, VAccount>,
    whitelisted_tokens: UnorderedSet<AccountId>,
    //guardians: UnorderedSet<AccountId>,
    state: RunningState,
}


const CONTRACT_ID: &str = "exchange.ref-dev.testnet";
const CONTRACT_ID_WRAP: &str = "wrap.testnet";
const CONTRACT_ID_FARM: &str = "farm110.ref-dev.testnet";

#[ext_contract(ext_exchange)]
pub trait RefExchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
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



#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: ValidAccountId/*, exchange_fee: u32, referral_fee: u32*/) -> Self {
        Self {
            owner_id: owner_id.as_ref().clone(),
            //exchange_fee,
            //referral_fee,
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

    //Para trocar near em wnear
    fn near_to_wrap(&self, receiver_id: AccountId, amount: String, msg: String) {
        //Registro de usuário
        log!("Entrei no near_to_wrap");
        /*
        ext_wrap::storage_deposit(
            &CONTRACT_ID_WRAP, // contract account id
            1250000000000000000000, // yocto NEAR to attach
            35_000_000_000_000 // gas to attach
        )
        .then(*/
            ext_wrap::near_deposit(
                &CONTRACT_ID_WRAP, // contract account id
                amount.parse().unwrap(), // yocto NEAR to attach
                3_000_000_000_000 // gas to attach
            )
        //)
        .then(
            ext_wrap::ft_transfer_call(
                receiver_id,//receiver_id,
                amount,
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
        35_000_000_000_000 // gas to attach
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
            180_000_000_000_000 // gas to attach
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

    

    //Main vault function
    pub fn add_to_vault(&self, account_id: ValidAccountId, vault_contract: ValidAccountId /*, amount: String, msg: String*/) -> String {

        ///////////////Sending wrap near to ref//////////////////
        //Getting user's near deposits.

        let acc = self.internal_get_account(account_id.as_ref());
        let mut x: u128 = 0; 
        if let Some(account) = acc {
            Some(
                x = account.storage_usage()
            )     
        } else {
            None
        };

        let amount: u128 = 10000000000000000000;//= x/2;1250000000000000000000
        log!("Esse é o amount do user que vai ser mandado para a ref: {}", amount);

        self.near_to_wrap(CONTRACT_ID.to_string(), amount.to_string(), "".to_string());
        
        
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
/**/

        ///////////////Adding liquidity to the pool///////////////
         
         
        let pool_id_to_add_liquidity = 193;
        let token_out1 = "eth.fakes.testnet".to_string();
        let token_out2 = "dai.fakes.testnet".to_string();
        let mut quantity_of_token1 = U128(0);
        let mut quantity_of_token2 = U128(0);
        let tokens = self.call_get_deposits(vault_contract);
        let deposits = tokens.find("wrap.testnet");

        for (key, val) in tokens.iter() {
            log!("entrou aqui fora " );
            if key.to_string() == token_out1 {quantity_of_token1 = *val; log!("entrou aqui " )};
            if key.to_string() == token_out2 {quantity_of_token2 = *val;};
        }
        //log!("{} {}",quantity_of_token1, quantity_of_token2 );
        self.call_add_liquidity(pool_id_to_add_liquidity, vec![quantity_of_token1, quantity_of_token2], None);
        /**/

        ///////////////Updating the user balance of tokens, near and lp///////////////



        ///////////////Staking new lp tokens///////////////



        ///////////////Claiming the reward///////////////  79928016015510



        "OK!".to_string()
    }








    /*
    pub fn withdraw_all(&self, seed_id: String, amount: U128, msg: String) -> Promise {
        //Registro de usuário
        log!("Entrei no withdraw_all");

        //Fazendo claim das rewards
        ext_farm::claim_reward_by_seed(
            seed_id,
            &CONTRACT_ID_FARM, // contract account id
            0, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        );
        
        //Fazendo unstake do lp
        ext_farm::withdraw_seed(
            seed_id,
            amount,
            msg,
            &CONTRACT_ID_FARM, // contract account id
            1, // yocto NEAR to attach
            180_000_000_000_000 // gas to attach
        );
        
        //Passando reward para o contrato de exchange



    }*/




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
