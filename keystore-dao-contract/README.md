# Keystore DAO Contract

A NEAR smart contract that manages keystore registration through DAO governance. Keystores run in TEE (Trusted Execution Environment) and need DAO approval to receive deterministic master keys from the NEAR MPC network.

## Overview

This contract enables:
1. TEE-based keystores to submit registration requests with attestation
2. DAO members to vote on keystore proposals
3. Approved keystores to receive access keys for MPC CKD (Chain Key Derivation)
4. All keystores to derive the same deterministic master secret

## Architecture

```
TEE Instance 1 ─┐
TEE Instance 2 ─┼─→ keystore-dao.outlayer.testnet → MPC Network → Same Secret
TEE Instance 3 ─┘
```

All approved keystores use the same account (`keystore-dao.outlayer.testnet`) to request secrets from MPC, ensuring they all receive the identical deterministic key.

## Build & Deploy

### Build Contract

```bash
# macOS build via docker
./build-docker.sh

# local build
./build.sh
# or
cargo near build
```

### Deploy to Testnet
```bash
# Deploy contract
near contract deploy dao.outlayer.testnet use-file res/keystore_dao_contract.wasm without-init-call network-config testnet sign-with-keychain send

# Initialize with DAO members

near call dao.outlayer.testnet new '{"owner_id": "owner.outlayer.testnet", "init_account_id": "init-keystore.outlayer.testnet", "dao
_members": ["zavodil.testnet"], "mpc_contract_id": "v1.signer-prod.testnet"}' --accountId dao.outlayer.testnet
```

### Update Collateral (Required for TEE verification)
```bash
# Get latest Intel collateral from:
# https://api.trustedservices.intel.com/sgx/certification/v4/

near call dao.outlayer.testnet update_collateral \
  '{"collateral": "{...json...}"}' \
  --accountId admin.testnet
```

## Usage Flow

### 1. Keystore Registration

Keystore running in TEE submits registration:
```bash
near call keystore-dao.outlayer.testnet submit_keystore_registration \
  '{
    "public_key": "ed25519:...",
    "tdx_quote_hex": "..."
  }' \
  --accountId init-keystore.outlayer.testnet \
  --gas 50000000000000
```

### 2. DAO Voting

DAO members vote on the proposal:
```bash
near call keystore-dao.outlayer.testnet vote_on_proposal \
  '{
    "proposal_id": 1,
    "approve": true
  }' \
  --accountId alice.testnet
```

### 3. Check Approval Status

```bash
near view keystore-dao.outlayer.testnet get_proposal '{"proposal_id": 1}'

near view keystore-dao.outlayer.testnet is_keystore_approved \
  '{"public_key": "ed25519:..."}'
```

### 4. MPC CKD Request

After approval, keystore can request CKD from MPC:
```bash
# This is done by the keystore using its approved key
near call v1.signer.testnet request_app_private_key \
  '{
    "request": {
      "app_public_key": "bls12381g1:...",
      "domain_id": 2
    }
  }' \
  --accountId keystore-dao.outlayer.testnet \
  --signWithKey ed25519:...
```

## Contract Methods

### Admin Methods
- `update_collateral(collateral)` - Update TDX verification collateral
- `add_approved_measurements(measurements, clear_others?)` - Add approved TDX measurement set (MRTD + RTMR0-3)
- `remove_approved_measurements(measurements)` - Remove a measurement set
- `add_dao_member(member)` - Add a DAO member
- `remove_dao_member(member)` - Remove a DAO member

### Public Methods
- `submit_keystore_registration(public_key, tdx_quote_hex)` - Submit TEE registration
- `vote_on_proposal(proposal_id, approve)` - Vote on a proposal

### View Methods
- `is_keystore_approved(public_key)` - Check if keystore is approved
- `get_proposal(proposal_id)` - Get proposal details
- `list_pending_proposals()` - List all pending proposals
- `get_approved_measurements()` - Get list of approved TDX measurement sets
- `is_measurements_approved(measurements)` - Check if a measurement set (MRTD + RTMR0-3) is approved
- `get_dao_members()` - Get DAO members list
- `get_config()` - Get contract configuration

## Environment Variables for Keystore Worker

```bash
# TEE Configuration
TEE_MODE=tdx
USE_TEE_REGISTRATION=true

# DAO Contract
KEYMASTER_DAO_CONTRACT=keystore-dao.outlayer.testnet

# Init account for gas payment
INIT_ACCOUNT_ID=init-keystore.outlayer.testnet
INIT_ACCOUNT_PRIVATE_KEY=ed25519:...

# MPC Configuration
MPC_CONTRACT_ID=v1.signer.testnet
MPC_DOMAIN_ID=2  # BLS12-381 domain
MPC_PUBLIC_KEY=bls12381g2:...  # Domain public key
```

## Security Considerations

1. **TEE Verification**: All registrations require valid TDX attestation with all 5 measurements (MRTD + RTMR0-3)
2. **DAO Governance**: Multiple members must approve each keystore
3. **Deterministic Keys**: All keystores derive the same secret from MPC
4. **Access Control**: Only approved keys can call MPC contract
5. **Collateral Updates**: Only owner can update Intel verification data

## Testing

```bash
# Run tests
cargo test

# Deploy to testnet and test full flow
./scripts/test_registration.sh
```

## License

MIT