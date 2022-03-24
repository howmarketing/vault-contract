use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{ext_contract, PromiseOrValue};

use crate::*;

/// Message parameters to receive via token function call.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
enum TokenReceiverMessage {
    /// Alternative to deposit + execute actions call.
    Execute {
        referral_id: Option<AccountId>,
        // TODO: remove if not used
        // List of sequential actions.
        //actions: Vec<Action>,
    },
}

#[ext_contract(ext_self)]
pub trait RefExchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
    );
    fn metadata(&mut self);
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Callback on receiving tokens by this contract.
    /// `msg` format is either "" for deposit or `TokenReceiverMessage`.
    #[allow(unreachable_code)]
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        _msg: String,
    ) -> PromiseOrValue<U128> {
        self.assert_contract_running();
        ext_self::metadata(
            env::current_account_id(),
            0,                      // yocto NEAR to attach
            Gas(5_000_000_000_000), // gas to attach
        );
        let token_in = env::predecessor_account_id();
        self.internal_deposit(&sender_id, &token_in, amount.into());
        PromiseOrValue::Value(U128(0))
    }
}
