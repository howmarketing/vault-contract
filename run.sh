# build wasm
./build.sh

#Deploy with near-dev
near dev-deploy --wasmFile ./target/wasm32-unknown-unknown/release/vault_contract.wasm

source neardev/dev-account.env
echo $CONTRACT_NAME

username=''
echo $username
 
#### Initialize contract
near call $CONTRACT_NAME new '{"owner_id":"'$username'", "vault_shares": 0}' --accountId $username

#### Register contract 

#At ref
near call $CONTRACT_NAME call_user_register '{"account_id": "'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME

#At the farm
near call farm.leopollum.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --deposit 0.1

#At near wrap
near call wrap.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.00125

#At ref.fakes
near call ref.fakes.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.00125

#### Register user and add value to contract
near call $CONTRACT_NAME storage_deposit '{"account_id": "'$username'", "registration_only": false}' --accountId $username --gas 300000000000000 --deposit 5

#### Swaping near to wnear and sending to ref.
near call $CONTRACT_NAME near_to_wrap '{"account_id": "'$username'", "receiver_id": "exchange.ref-dev.testnet", "amount": "10000000000000000000000", "msg": ""}' --accountId $username --gas 300000000000000 

#### Swap, add liquidity, save new lp user balance, stake, claim, withdraw
near call $CONTRACT_NAME add_to_vault '{"account_id": "'$username'", "vault_contract": "'$CONTRACT_NAME'"}' --accountId $username --gas 300000000000000 --deposit 0.01

#### Withdraw the farm reward.
# near call $CONTRACT_NAME withdraw_of_reward '{"vault_contract": "'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.000000000000000000000001

#### Unstake, swap to wnear and send it to vault contract.
# near call $CONTRACT_NAME withdraw_all '{"seed_id": "exchange.ref-dev.testnet@193", "msg": "", "vault_contract": "'$CONTRACT_NAME'", "account_id": "'$username'"}' --accountId $username --gas 300000000000000 
# near call $CONTRACT_NAME withdraw_all_2 '{"vault_contract": "'$CONTRACT_NAME'", "account_id": "'$username'"}' --accountId $username --gas 300000000000000 


#### Start croncat tasks responsible for auto-compound 
#near call manager_v1.croncat.testnet create_task '{"contract_id": "'$CONTRACT_NAME'", "function_id": "withdraw_of_reward","cadence": " */19 * * * * *","recurring": true,"deposit": "1","gas": 230000000000000}' --accountId $CONTRACT_NAME --amount 5 --gas 300000000000000 
#near call manager_v1.croncat.testnet create_task '{"contract_id": "'$CONTRACT_NAME'", "function_id": "auto_function_1","cadence": " */20 * * * * *","recurring": true,"deposit": "0","gas": 200000000000000}' --accountId $CONTRACT_NAME --amount 5 --gas 300000000000000
#near call manager_v1.croncat.testnet create_task '{"contract_id": "'$CONTRACT_NAME'", "function_id": "auto_function_2","cadence": " */20 * * * * *","recurring": true,"deposit": "0","gas": 230000000000000}' --accountId $CONTRACT_NAME --amount 5 --gas 300000000000000

#near call $CONTRACT_NAME withdraw_of_reward '{}' --accountId $CONTRACT_NAME --gas 230000000000000 --deposit 0.000000000000000000000001

#near call $CONTRACT_NAME auto_function_1 '{}' --accountId $CONTRACT_NAME --gas 200000000000000 
#near call $CONTRACT_NAME auto_function_2 '{}' --accountId $CONTRACT_NAME --gas 230000000000000 