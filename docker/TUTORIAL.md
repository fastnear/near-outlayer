# Check proper cvmUuid 
.phala/config

# build keystore 
./scripts/build_and_push_keystore.sh zavodil v1.0.3

# deploy keystore to phala
cd docker
phala deploy --name outlayer-testnet-keystore --compose docker-compose.keystore-phala.yml --env-file .env.testnet-keystore-phala --vcpu 2 --memory 2G --disk-size 20G --kms-id phala-prod10

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
  '{"rtmr3":"0ea5ecbe56b001163dff68b320c5759ac6355ac95097ccfdb99d556ca525ec317104a5f1ffc8008f9bdf63173886bed7"}' \
  --accountId owner.outlayer.testnet \
  --networkId testnet

# check whitelist
near contract call-function as-read-only worker.outlayer.testnet get_approved_rtmr3 json-args {} network-config testnet now

# check logs
curl 'https://f53d94690545c6f6ea877f471482822f406bf29f-8090.dstack-pha-prod9.phala.network/logs/near-offshore-worker?since=0&until=0&follow=true&text=true&timestamps=true&bare=true'
