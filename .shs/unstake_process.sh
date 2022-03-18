source neardev/dev-account.env
echo $CONTRACT_NAME

username=''
echo $username
#### Unstake, swap to wnear and send it to vault contract.
near call $CONTRACT_NAME withdraw_all '{"seed_id": "exchange.ref-dev.testnet@193", "msg": "", "vault_contract": "'$CONTRACT_NAME'", "account_id": "'$username'"}' --accountId $username --gas 300000000000000 
near call $CONTRACT_NAME withdraw_all_2 '{"vault_contract": "'$CONTRACT_NAME'", "account_id": "'$username'"}' --accountId $username --gas 300000000000000 
