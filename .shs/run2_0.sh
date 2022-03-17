# build wasm
./build.sh

#Deploy with near-dev
near dev-deploy --wasmFile ../res/vault_contract.wasm

source neardev/dev-account.env
echo $CONTRACT_NAME

username='leopollum.testnet'
echo $username
 

# initializes the contract and registers the necessary 
#./initialize.sh

# storage_deposit + near_to_wrap + add_to_vault   
#./stake_process.sh

# withdraw_all + withdraw_all_2
#./unstake_process.sh
 
 
#1000000000000000000


#### Functions managed by auto-compound
near call $CONTRACT_NAME claim_reward '{}' --accountId $CONTRACT_NAME --gas 230000000000000 --deposit 0.000000000000000000000001

near call $CONTRACT_NAME withdraw_of_reward '{}' --accountId $CONTRACT_NAME --gas 230000000000000 --deposit 0.000000000000000000000001


near call $CONTRACT_NAME auto_function_1 '{"farm_id": "exchange.ref-dev.testnet@193#6"}' --accountId $CONTRACT_NAME --gas 200000000000000 
near call $CONTRACT_NAME auto_function_2 '{}' --accountId $CONTRACT_NAME --gas 230000000000000 
