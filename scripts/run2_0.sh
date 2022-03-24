# build wasm
./build.sh

#Deploy with near-dev
near dev-deploy --wasmFile ../res/vault_contract.wasm

source neardev/dev-account.env
echo $CONTRACT_NAME

username='leopollum.testnet'
echo $username
 
near call $CONTRACT_NAME user_total_near_deposited '{"account_id": "leopollum.testnet"}' --accountId leopollum.testnet --gas 230000000000000 --deposit 0.000000000000000000000001
# initializes the contract and registers the necessary 
#./initialize.sh


# storage_deposit + near_to_wrap + add_to_vault   
./stake_process.sh

# withdraw_all + withdraw_all_2
./unstake_process.sh
 
near call $CONTRACT_NAME user_total_near_deposited '{"account_id": "leopollum.testnet"}' --accountId leopollum.testnet --gas 230000000000000 --deposit 0.000000000000000000000001

#near call $CONTRACT_NAME user_total_near_deposited '{"account_id": "leopollum.testnet"}' --accountId leopollum.testnet --gas 230000000000000 --deposit 0.000000000000000000000001


#rm -rf /mnt/c/Users/jonal/Desktop/Near/git/dev_user_initial_deposit/vault-contract/.shs/neardev/dev-account
#rm -rf /mnt/c/Users/jonal/Desktop/Near/git/dev_user_initial_deposit/vault-contract/.shs/neardev/dev-account.env
#rm -rf /mnt/c/Users/jonal/Desktop/Near/git/dev_user_initial_deposit/vault-contract/.shs/neardev


#near call $CONTRACT_NAME user_total_near_deposited '{"account_id": "leopollum.testnet"}' --accountId leopollum.testnet --gas 230000000000000 --deposit 0.000000000000000000000001

 
#1000000000000000000

