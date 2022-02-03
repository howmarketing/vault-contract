#Chamando o script de build que vai atualizar o vault_contract.wasm
./build.sh

#Deploy do contrato em uma conta dev
near dev-deploy --wasmFile ./target/wasm32-unknown-unknown/release/vault_contract.wasm

#Usando a conta deployada como variável ambiente
source neardev/dev-account.env
echo $CONTRACT_NAME

#### User register and near deposit in vault contract.
#near call $CONTRACT_NAME storage_deposit '{"account_id": "leopollum.testnet", "registration_only": false}' --accountId leopollum.testnet --gas 300000000000000 --deposit 0.1

#### Swaping near to wnear and seending to ref.
#near call $CONTRACT_NAME near_to_wrap '{"account_id": "leopollum.testnet", "receiver_id": "exchange.ref-dev.testnet", "amount": "100000000000000000000000", "msg": ""}' --accountId leopollum.testnet --gas 300000000000000 

#### Swap, add liquidity, save new lp user balance, stake, claim, withdraw
#near call $CONTRACT_NAME add_to_vault '{"account_id": "leopollum.testnet", "vault_contract": "'$CONTRACT_NAME'"}' --accountId leopollum.testnet --gas 300000000000000 --deposit 0.01

#### Withdraw the farm reward.
#near call $CONTRACT_NAME withdraw_of_reward '{"vault_contract": "'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME --gas 300000000000000 --deposit 0.000000000000000000000001

#### Unstake, swap to wnear and send it to vault contract.
#near call $CONTRACT_NAME withdraw_all '{"seed_id": "exchange.ref-dev.testnet@193", "amount": "173904470178311485196", "msg": "", "vault_contract": "'$CONTRACT_NAME'", "account_id": "leopollum.testnet"}' --accountId leopollum.testnet --gas 300000000000000 








#####Inicializando o contrato #####
#near call $CONTRACT_NAME new '{"owner_id":"leopollum.testnet", "vault_shares": 0}' --accountId leopollum.testnet

##### Chamando função de registrar usuário #####
#near call $CONTRACT_NAME call_user_register '{"account_id": "'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME


###### Transferindo token para o contrato de vault #####
#near call dai.fakes.testnet ft_transfer_call '{"receiver_id": "'exchange.ref-dev.testnet'", "amount": "851830910754170900", "msg": ""}' --account_id=$CONTRACT_NAME --amount=0.000000000000000000000001 --gas=300000000000000
#near call eth.fakes.testnet ft_transfer_call '{"receiver_id": "'exchange.ref-dev.testnet'", "amount": "2000000000000000000", "msg": ""}' --account_id=$CONTRACT_NAME --amount=0.000000000000000000000001 --gas=300000000000000


##### Chamando função de pegar metadata #####
#near call $CONTRACT_NAME call_meta '{}' --accountId $CONTRACT_NAME


##### Mandando wnear pra ref #####
#near call wrap.testnet near_deposit '{}' --accountId $CONTRACT_NAME --deposit 10
#near call wrap.testnet ft_transfer_call '{"receiver_id": "exchange.ref-dev.testnet","amount": "10000000000000000000000000","msg": ""}' --accountId $CONTRACT_NAME --deposit 0.000000000000000000000001 --gas 300000000000000


##### tirando near da ref #####
#near call wrap.testnet near_withdraw '{"amount":"1000000000000000000000000"}' --accountId leopollum.testnet --deposit 0.000000000000000000000001


##### Wrap de near #####
#near call $CONTRACT_NAME near_to_wrap '{"receiver_id": "leopollum.testnet", "amount": "100000000000000000000000", "msg": ""}' --accountId $CONTRACT_NAME  --deposit 0.000000000000000000000001 --gas 300000000000000


##### Swap de wnear para rft #####
#near call $CONTRACT_NAME call_swap '{"actions": [{"pool_id": 4,"token_in": "wrap.testnet","token_out": "rft.tokenfactory.testnet", "amount_in": "500000000000000000000000", "min_amount_out": "0"}]}' --accountId $CONTRACT_NAME  --deposit 0.000000000000000000000001 --gas 300000000000000


##### Ja temos wrap near la, agora add liquidez #####
#near call $CONTRACT_NAME call_add_liquidity '{"pool_id": 193, "amounts": ["2259981532704057266", "2359953913253356717"]}' --accountId $CONTRACT_NAME  --deposit 0.000000000000000000000001 --gas 300000000000000


##### Fazendo stake#####
#near call $CONTRACT_NAME call_stake '{"receiver_id": "farm110.ref-dev.testnet", "token_id": ":193", "amount": "4608525179932846610798", "msg": ""}' --accountId $CONTRACT_NAME  --deposit 0.000000000000000000000001 --gas 300000000000000
#near call farm110.ref-dev.testnet list_user_seeds '{"account_id":"'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME


##### Fazendo claim das rewards ##### 
#near call $CONTRACT_NAME call_claim '{"seed_id": "exchange.ref-dev.testnet@193"}' --account_id=$CONTRACT_NAME  --gas 300000000000000

#near view farm110.ref-dev.testnet list_rewards '{"account_id": "'$CONTRACT_NAME'"}' --account_id=$CONTRACT_NAME
#near view farm110.ref-dev.testnet get_unclaimed_reward '{"account_id": "'$CONTRACT_NAME'", "farm_id":"exchange.ref-dev.testnet@193#0"}' --accountId $CONTRACT_NAME


##### Fazendo Unstake #####
#near call $CONTRACT_NAME call_unstake '{"seed_id": "exchange.ref-dev.testnet@193", "amount":"100000" , "msg":""}' --account_id=$CONTRACT_NAME  --gas 300000000000000 --deposit 0.000000000000000000000001


##### Withdraw de rewards #####
#near call $CONTRACT_NAME call_withdraw_reward '{"token_id": "ref.fakes.testnet", "amount":"2140709894097076593" , "unregister":"false"}' --account_id=leopollum.testnet  --gas 300000000000000 --deposit 0.000000000000000000000001

#near call $CONTRACT_NAME call_get_deposits '{"account_id":"'$CONTRACT_NAME'"}' --accountId $CONTRACT_NAME  --gas 300000000000000 --deposit 0.000000000000000000000001



#near call exchange.ref-dev.testnet remove_liquidity '{"pool_id": 193, "shares": "100000", "min_amounts": ["1", "1"]}' --accountId $CONTRACT_NAME --amount 0.000000000000000000000001

#near call exchange.ref-dev.testnet withdraw '{"token_id": "", "amount": "900000000000"}' --accountId $CONTRACT_NAME --amount 0.000000000000000000000001
