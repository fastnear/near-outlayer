# Check proper cvmUuid 
.phala/config

# build keystore 
./scripts/build_and_push_keystore.sh zavodil v1.0.3^C

# deploy keystore to phala
cd docker
phala deploy --name outlayer-testnet-keystore --compose docker-compose.keystore-phala.yml --env-file .env.testnet-keystore-phala --vcpu 2 --memory 2G --disk-size 20G --kms-id phala-prod10

# set KEYSTORE_BASE_URL based on keystore deployment
/docker/.env.testnet-worker-phala

# build worker 
./scripts/build_and_push_phala.sh zavodil v0.1.1

# deploy worker to phala
cd docker 
phala deploy --name outlayer-testnet-worker --compose docker-compose.phala.yml --env-file .env.testnet-worker-phala --vcpu 2 --memory 2G --disk-size 20G --kms-id phala-prod10

# whitelist RTMR3 after code updates
run worker, find rtmr3 in logs

near call worker.outlayer.testnet add_approved_rtmr3 \
  '{"rtmr3":"5a2e4e7f8a5e1e5c3c2e4e7f8a5e1e5c3c2e4e7f8a5e1e5c3c2e4e7f8a5e1e5c3c2e4e7f8a5e1e5c3c"}' \
  --accountId owner.outlayer.testnet \
  --networkId testnet

# check whitelist
near contract call-function as-read-only worker.outlayer.testnet get_approved_rtmr3 json-args {} network-config testnet now