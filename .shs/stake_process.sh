source neardev/dev-account.env
echo $CONTRACT_NAME

username=''
echo $username

#### Register user and add value to contract
near call $CONTRACT_NAME storage_deposit '{"account_id": "'$username'", "registration_only": false}' --accountId $username --gas 300000000000000 --deposit 5.5

#### Swaping near to wnear and sending to ref.
near call $CONTRACT_NAME near_to_wrap '{"account_id": "'$username'", "receiver_id": "exchange.ref-dev.testnet", "amount": "10000000000000000000000", "msg": ""}' --accountId $username --gas 300000000000000 

#### Swap, add liquidity, save new lp user balance, stake, claim, withdraw
near call $CONTRACT_NAME add_to_vault '{"account_id": "'$username'", "vault_contract": "'$CONTRACT_NAME'"}' --accountId $username --gas 300000000000000 --deposit 0.01

