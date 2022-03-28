source neardev/dev-account.env
echo $CONTRACT_NAME

username='leopollum.testnet'
echo $username

#### Initialize contract
near call $CONTRACT_NAME new '{"owner_id":"'$username'", "vault_shares": 0,
"pool_token1": "eth.fakes.testnet", "pool_token2": "dai.fakes.testnet", 
"pool_id_token1_wrap": 356, "pool_id_token2_wrap": 231, 
"pool_id_token1_reward": 321, "pool_id_token2_reward": 326,
"farm_id": "exchange.ref-dev.testnet@193#6", "reward_token": "ref.fakes.testnet",
"pool_id":193, "seed_id":"exchange.ref-dev.testnet@193"}' --accountId $username

#### Register contract 

#At ref
near call $CONTRACT_NAME call_user_register '{"account_id": "'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME

#At the farm
near call farm.leopollum.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --deposit 0.1

#At near wrap
near call wrap.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.00125

#At ref.fakes
near call ref.fakes.testnet storage_deposit '{"account_id": "'$CONTRACT_NAME'", "registration_only": false}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.00125

 