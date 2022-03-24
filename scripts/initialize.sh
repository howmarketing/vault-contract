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

 