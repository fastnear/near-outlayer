# Check proper cvmUuid 
.phala/config

# build keystore 
####### old ./scripts/build_and_push_keystore.sh zavodil v1.0.3
./scripts/build_and_push_keystore_tee.sh zavodil latest

# deploy keystore to phala
cd docker
phala deploy --name outlayer-testnet-keystore --compose docker-compose.keystore-phala.yml --env-file .env.testnet-keystore-phala --vcpu 1 --memory 1G --disk-size 10G --kms-id phala-prod10

phala cvms create --name outlayer-testnet-keystore --compose ./docker-compose.keystore-phala.yml --env-file ./.env.testnet-keystore-phala  --vcpu 1 --memory 1G --disk-size 10G

# set KEYSTORE_BASE_URL based on keystore deployment
/docker/.env.testnet-worker-phala

# build worker-only
./scripts/build_and_push_phala.sh zavodil latest worker

# build worker-compiler 
./scripts/build_and_push_phala.sh zavodil latest worker-compiler

# deploy worker to phala
cd docker 
phala deploy --name outlayer-testnet-worker --compose docker-compose.phala.yml --env-file .env.testnet-worker-phala --vcpu 1 --memory 1G --disk-size 3G --kms-id phala-prod10

# deploy worker-compiler to phala
cd docker 
phala deploy --name outlayer-testnet-worker-compiler --compose docker-compose.worker-compiler.phala.yml --env-file .env.testnet-worker-compiler-phala --vcpu 1 --memory 4G --disk-size 10G --kms-id phala-prod10

# whitelist RTMR3 after code updates
run worker, find rtmr3 in logs

near call worker.outlayer.testnet add_approved_rtmr3 \
  '{"rtmr3":"3532fc9f9ea02061b67f89904e27c005b4cac86f58f439d224cc7538cc9e158975639ed5ae6d7f68943f35fd8b204ddf"}' \
  --accountId owner.outlayer.testnet \
  --networkId testnet

near contract call-function as-transaction dao.outlayer.testnet add_approved_rtmr3 json-args '{"rtmr3": "3dc2180c4bb80d112bdfc3b24ee3777d86e55fe24331f8b58749f8786ad04294e84a70af5b8b6a38103577104d200f3c"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send

# check whitelist
near contract call-function as-read-only worker.outlayer.testnet get_approved_rtmr3 json-args {} network-config testnet now

# check logs
curl 'https://f53d94690545c6f6ea877f471482822f406bf29f-8090.dstack-pha-prod9.phala.network/logs/near-offshore-worker?since=0&until=0&follow=true&text=true&timestamps=true&bare=true'
