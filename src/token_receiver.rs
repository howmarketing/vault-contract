
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{ PromiseOrValue,ext_contract};


use crate::*;



/// Message parameters to receive via token function call.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
enum TokenReceiverMessage {
    /// Alternative to deposit + execute actions call.
    Execute {
        referral_id: Option<ValidAccountId>,
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
    fn metadata(
        &mut self,
    );

}



/*
pub fn my_method(&self) -> Promise {
    ext_1::metadata()
}
*/

/*
impl Contract {
    /// Executes set of actions on virtual account.
    /// Returns amounts to send to the sender directly.
    fn internal_direct_actions(
        &mut self,
        token_in: AccountId,
        amount_in: Balance,
        referral_id: Option<AccountId>,
        actions: &[Action],
    ) -> Vec<(AccountId, Balance)> {

        // let @ be the virtual account
        let mut account: Account = Account::new(&String::from(VIRTUAL_ACC));

        account.deposit(&token_in, amount_in);
        let _ = self.internal_execute_actions(
            &mut account,
            &referral_id,
            &actions,
            ActionResult::Amount(U128(amount_in)),
        );

        let mut result = vec![];
        for (token, amount) in account.tokens.to_vec() {
            if amount > 0 {
                result.push((token.clone(), amount));
            }
        }
        account.tokens.clear();

        result
    }

}
*/

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Callback on receiving tokens by this contract.
    /// `msg` format is either "" for deposit or `TokenReceiverMessage`.
    #[allow(unreachable_code)]
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.assert_contract_running();
        ext_self::metadata(        
            &env::current_account_id(),
            0, // yocto NEAR to attach
            5_000_000_000_000 // gas to attach
        );
        let token_in = env::predecessor_account_id();
        //if msg.is_empty() {
            // Simple deposit.
            self.internal_deposit(sender_id.as_ref(), &token_in, amount.into());
            PromiseOrValue::Value(U128(0))
       // } 
       /*else {
            // instant swap
            let message =
                serde_json::from_str::<TokenReceiverMessage>(&msg).expect(ERR28_WRONG_MSG_FORMAT);
            match message {
                TokenReceiverMessage::Execute {
                    referral_id,
                    actions,
                } => {
                    let referral_id = referral_id.map(|x| x.to_string());
                    let out_amounts = self.internal_direct_actions(
                        token_in,
                        amount.0,
                        referral_id,
                        &actions,
                    );
                    for (token_out, amount_out) in out_amounts.into_iter() {
                        self.internal_send_tokens(sender_id.as_ref(), &token_out, amount_out);
                    }
                    // Even if send tokens fails, we don't return funds back to sender.
                    PromiseOrValue::Value(U128(0))
                }
            }
        }*/
    }
}