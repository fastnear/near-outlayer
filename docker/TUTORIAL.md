# Check proper cvmUuid 
.phala/config

# build keystore 
####### old ./scripts/build_and_push_keystore.sh zavodil v1.0.3
./scripts/build_and_push_keystore_tee.sh zavodil latest

# deploy keystore to phala
cd docker
phala deploy --name outlayer-testnet-keystore --compose docker-compose.keystore-phala.yml --env-file .env.testnet-keystore-phala --vcpu 1 --memory 1G --disk-size 1G --kms-id phala-prod10

phala cvms create --name outlayer-testnet-keystore --compose ./docker-compose.keystore-phala.yml --env-file ./.env.testnet-keystore-phala  --vcpu 1 --memory 1G --disk-size 1G

# set KEYSTORE_BASE_URL based on keystore deployment
/docker/.env.testnet-worker-phala

# build worker-only
./scripts/build_and_push_phala.sh zavodil latest worker

# build worker-compiler 
./scripts/build_and_push_phala.sh zavodil latest worker-compiler

# deploy worker to phala
cd docker 
phala deploy --name outlayer-testnet-worker --compose docker-compose.phala.yml --env-file .env.testnet-worker-phala --vcpu 1 --memory 1G --disk-size 2G --kms-id phala-prod10

# deploy worker-compiler to phala
cd docker 
phala deploy --name outlayer-testnet-worker-compiler --compose docker-compose.worker-compiler.phala.yml --env-file .env.testnet-worker-compiler-phala --vcpu 1 --memory 4G --disk-size 10G --kms-id phala-prod10

# whitelist RTMR3 after code updates
run worker, find rtmr3 in details/worker on a phala dashboard, or find in worker's logs

near contract call-function as-transaction worker.outlayer.testnet add_approved_rtmr3 json-args '{"rtmr3":"e664cdc1f6f925bd40e2973c7b975440a46169fafa84f58a1fbb72d7c2357cb0e5acc13a35f043a70ed66db2b684d23d", "clear_others": true}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send

near contract call-function as-transaction dao.outlayer.testnet add_approved_rtmr3 json-args '{"rtmr3": "45a9f2f9c788c5c052ac1bbd295c6c95de59658799a1c475aebd009e98047d748d525fdb60b2cb0a4e31b7c822de2681"}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as owner.outlayer.testnet network-config testnet sign-with-keychain send

# check whitelist
near contract call-function as-read-only worker.outlayer.testnet get_approved_rtmr3 json-args {} network-config testnet now

# check logs
curl 'https://f53d94690545c6f6ea877f471482822f406bf29f-8090.dstack-pha-prod9.phala.network/logs/near-offshore-worker?since=0&until=0&follow=true&text=true&timestamps=true&bare=true'
