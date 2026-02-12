# Scripts

Deployment, build, and utility scripts for NEAR OutLayer.

## Deployment

| Script | Description |
|--------|-------------|
| `deploy_phala.sh` | **Main deployment script** — builds Docker image, deploys to Phala Cloud, extracts TDX measurements, adds to contract, restarts CVM |
| `build_and_push_phala.sh` | Build and push Docker images for worker/coordinator |
| `build_and_push_keystore.sh` | Build and push keystore Docker image |
| `build_and_push_keystore_tee.sh` | Build and push keystore with TEE support |
| `build_local_fast.sh` | Fast local build (skips Docker) |
| `dry_run_docker_build.sh` | Test Docker build without deploying |
| `inspect_docker_context.sh` | Debug Docker build context |
| `verify_built_image.sh` | Verify Docker image integrity |

## Runtime

| Script | Description |
|--------|-------------|
| `run_coordinator.sh` | Start coordinator locally |
| `run_worker.sh` | Start worker locally |
| `run_worker_local.sh` | Start worker with local config |
| `run_keystore.sh` | Start keystore worker locally |

## Maintenance

| Script | Description |
|--------|-------------|
| `update_collateral.sh` | Update Intel TDX collateral on contract (testnet) |
| `update_collateral_mainnet.sh` | Update Intel TDX collateral on contract (mainnet) |
| `fetch_intel_collateral.sh` | Fetch collateral from Intel PCS API |
| `clear_redis_queue.sh` | Clear Redis task queue |
| `clear_wasm_cache.sh` | Clear compiled WASM cache |
| `storage_stats.sh` | Show storage usage statistics |

## Testing

| Script | Description |
|--------|-------------|
| `test_execution_request.sh` | Send test execution request |
| `test_compiler.sh` | Test WASM compilation pipeline |
| `test_register_contract.sh` | Test register-contract flow |
| `check_coordinator_security.sh` | Security audit checks |
