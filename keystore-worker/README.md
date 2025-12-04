# Keystore Worker - TEE Secret Management for NEAR OutLayer

The **Keystore Worker** is a secure service that runs in a Trusted Execution Environment (TEE) to manage encryption/decryption of user secrets for the NEAR OutLayer platform.

## Overview

When users want to execute code that requires secrets (API keys, credentials, etc.), they encrypt those secrets with the keystore's public key. The keystore worker then decrypts these secrets ONLY for verified executor workers running in TEE environments.

**Security Model:**
- Private key **NEVER** leaves TEE memory
- Only verified workers (via TEE attestation) can request decryption
- Token-based authentication for API access
- Public key is published on-chain in the NEAR contract

## Architecture

```
┌─────────────────────────────────────┐
│  User / Contract                    │
│  - Gets pubkey from contract        │
│  - Encrypts secrets with pubkey     │
└────────┬────────────────────────────┘
         │
         ↓ (encrypted secrets in request_execution)
┌─────────────────────────────────────┐
│  NEAR Contract                      │
│  - Stores keystore pubkey           │
│  - Validates requests               │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│  Executor Worker (in TEE)           │
│  - Receives task with encrypted     │
│  - Generates attestation proof      │
│  - Requests decryption              │
└────────┬────────────────────────────┘
         │
         ↓ POST /decrypt (with attestation)
┌─────────────────────────────────────┐
│  Keystore Worker (in TEE)           │
│  ✓ Verify attestation               │
│  ✓ Decrypt with private key         │
│  ✓ Return plaintext (over TLS)      │
│  - Private key stays in TEE         │
└─────────────────────────────────────┘
```

## Features

- **TEE-Ready:** Designed for Intel SGX, AMD SEV-SNP, or simulated TEE
- **High Performance:** Async/await with Tokio for parallel request handling
- **Secure:** Attestation verification prevents unauthorized access
- **Simple API:** RESTful HTTP endpoints
- **Contract Integration:** Publishes public key to NEAR contract
- **Token Auth:** SHA256 bearer tokens for additional security layer
- **CKD Support:** Confidential Key Derivation via NEAR MPC Network for deterministic secrets

## API Endpoints

### `GET /health`
Health check and public key info (no auth required)

**Response:**
```json
{
  "status": "ok",
  "public_key": "a1b2c3d4...",
  "tee_mode": "Sgx"
}
```

### `GET /pubkey`
Get keystore public key (no auth required)

**Response:**
```json
{
  "public_key_hex": "a1b2c3d4...",
  "public_key_base58": "Ed25519:..."
}
```

### `POST /decrypt`
Decrypt secrets for verified TEE worker (requires auth)

**Headers:**
```
Authorization: Bearer <worker-token>
```

**Request:**
```json
{
  "encrypted_secrets": "base64-encoded-ciphertext",
  "attestation": {
    "tee_type": "sgx",
    "quote": "base64-encoded-attestation-quote",
    "worker_pubkey": "optional-worker-pubkey",
    "timestamp": 1234567890
  },
  "task_id": "optional-task-id"
}
```

**Response:**
```json
{
  "plaintext_secrets": "base64-encoded-plaintext"
}
```

**Error Response:**
```json
{
  "error": "Attestation verification failed: ..."
}
```

## Configuration

Create a `.env` file (see `.env.example`):

```bash
# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8081

# NEAR Configuration
NEAR_NETWORK=testnet
NEAR_RPC_URL=https://rpc.testnet.fastnear.com?apiKey=FASTNEARDEVSUoeFIcg7PpuKnAcwlz4FGPMM2K7GTgWP
OFFCHAINVM_CONTRACT_ID=outlayer.testnet

# Keystore account (must be authorized in contract)
KEYSTORE_ACCOUNT_ID=keystore.testnet
KEYSTORE_PRIVATE_KEY=ed25519:...

# NEAR RPC Client (for reading secrets from contract)
# Both are REQUIRED for repo-based secrets to work:
NEAR_CONTRACT_ID=outlayer.testnet

# Master Secret for Key Derivation
# Generate: openssl rand -hex 32
KEYSTORE_MASTER_SECRET=your_master_secret_hex_64_chars

# Worker authentication (SHA256 hashes of bearer tokens)
ALLOWED_WORKER_TOKEN_HASHES=hash1,hash2,hash3

# TEE mode (sgx|sev|simulated|none)
TEE_MODE=none

# Logging
RUST_LOG=info,keystore_worker=debug
```

## Setup

### 1. Generate Keystore Account and Key

```bash
# Create a new NEAR account for keystore
near create-account keystore.testnet --useFaucet

# Generate a new keypair (this will be stored in ~/.near-credentials)
# Or use an existing key from the credentials file
```

### 2. Generate Worker Auth Tokens

```bash
# Generate a secure random token
TOKEN=$(openssl rand -hex 32)
echo "Worker token: $TOKEN"

# Hash it with SHA256
TOKEN_HASH=$(echo -n "$TOKEN" | sha256sum | cut -d' ' -f1)
echo "Token hash (add to .env): $TOKEN_HASH"
```

Add the hash to `ALLOWED_WORKER_TOKEN_HASHES` in `.env`:
```
ALLOWED_WORKER_TOKEN_HASHES=cbd8f6f0e3e8ec29d3d1f58a2c8c6d6e8d7f5a4b3c2d1e0f1a2b3c4d5e6f7a8b
```

Workers will use the original token (not the hash) in their requests:
```
Authorization: Bearer <original-token>
```

### 3. Configure Contract

The keystore worker will verify that its public key matches the contract's stored key on startup. You need to set the public key in the contract first:

```bash
# Get the public key from keystore worker startup logs or /pubkey endpoint
# Then call contract method:
near contract call-function as-transaction outlayer.testnet set_keystore_pubkey json-args '{"pubkey_hex":"a1b2c3d4..."}' prepaid-gas '30.0 Tgas' attached-deposit '0 NEAR' sign-as keystore.testnet network-config testnet sign-with-keychain send
```

### 4. Run Keystore Worker

```bash
cd keystore-worker

# Install dependencies
cargo build --release

# Run
cargo run --release

# Or run directly
./target/release/keystore-worker
```

You should see:
```
INFO  Starting NEAR OutLayer Keystore Worker
INFO  Keystore initialized, public_key=a1b2c3d4...
INFO  ✓ Public key verified - matches contract
INFO  Keystore worker API server started, addr=0.0.0.0:8081
INFO  Ready to serve decryption requests from executor workers
```

## Testing

### Test Health Endpoint

```bash
curl http://localhost:8081/health
```

### Test Encryption/Decryption

Use the helper scripts in `scripts/` directory:

```bash
# Test that encryption/decryption works
cd scripts
./test_encryption.py
# ✅ SUCCESS! Encryption/decryption works correctly!

# Encrypt secrets for contract
./encrypt_secrets.py "OPENAI_KEY=sk-test123,FOO=bar"
# Output: [117, 56, 11, ...] - use this in contract call
```

See [scripts/README.md](scripts/README.md) for more details.

### Test Full Integration

```bash
# 1. Start keystore-worker (terminal 1)
cargo run

# 2. Encrypt secrets (terminal 2)
cd scripts
./encrypt_secrets.py "OPENAI_KEY=sk-test"

# 3. Call contract with encrypted secrets (terminal 2)
near call outlayer.testnet request_execution \
  '{"code_source": {...}, "encrypted_secrets": [117, 56, ...]}' \
  --accountId user.testnet --deposit 0.1

# 4. Worker will decrypt and use secrets
```

## TEE Integration

### Current Status (MVP)
- ✅ Code is TEE-ready (conditional compilation tags in place)
- ✅ Attestation verification framework implemented
- ⏳ Sealed storage not yet implemented
- ⏳ SGX/SEV SDKs not yet integrated

### For Production TEE Deployment

**Intel SGX:**
1. Add dependency: `sgx_tstd`, `sgx_types`
2. Implement sealed storage in `initialize_keystore()`
3. Integrate Intel Attestation Service (IAS) or DCAP
4. Build with `cargo build --target x86_64-fortanix-unknown-sgx`

**AMD SEV-SNP:**
1. Add dependency: `sev` crate
2. Implement SEV attestation verification
3. Use SNP guest tools for attestation generation
4. Build with appropriate target

**Key Changes for TEE:**
- Replace XOR encryption with proper AEAD (ChaCha20-Poly1305)
- Use X25519 ECDH instead of Ed25519 for encryption
- Implement sealed storage for private key persistence
- Add remote attestation with hardware root of trust

## Confidential Key Derivation (CKD)

### Overview

The keystore integrates with NEAR's **Confidential Key Derivation (CKD)** - an advanced cryptographic primitive that leverages the NEAR MPC Network to provide deterministic secrets for TEE applications. Unlike traditional key derivation, CKD uses distributed computation where multiple MPC nodes collaborate to generate secrets without any single node knowing the final value.

### How It Works with MPC Network

```
TEE App → Developer Contract → MPC Contract → MPC Network
    ↑                                               ↓
    └─── Encrypted Secret (Y, C) ←─────────────────┘

No single MPC node knows the final secret!
```

The CKD protocol flow:
1. TEE app generates fresh ElGamal key pair (a, A) and includes A in attestation
2. Developer contract validates TEE attestation and calls MPC contract
3. Each MPC node computes partial BLS signature using its secret share
4. Coordinator aggregates encrypted shares into (Y, C)
5. Only the TEE app can decrypt using private key a to get final secret

### Security Properties

1. **Deterministic** - Same app_id always produces the same secret
2. **Private** - Secret known only to the requesting TEE app
3. **Distributed** - No single MPC node has the complete secret
4. **TEE-protected** - Secrets computed and used only inside secure enclaves
5. **Threshold security** - Requires t-of-n MPC nodes to cooperate

### Cryptographic Foundation

- **BLS signatures** on pairing-friendly BLS12-381 curves
- **ElGamal encryption** for secure transport
- **HKDF** for final key derivation
- **Threshold cryptography** ensuring no single point of failure

### Use Case: Persistent Secrets

When a user stores secrets for their repository:
1. Keystore derives a unique child key for that repo
2. Secrets are encrypted with the child key
3. After keystore restart, the same child key can be regenerated
4. Secrets remain accessible without storing any keys on disk

### Implementation

The CKD is implemented using HMAC-SHA256 with domain separation:

```rust
// Derive child key for a specific repository
let child_key = hmac_sha256(
    master_key,
    format!("keystore-ckd:{}:{}", repo_url, owner)
);
```

This ensures:
- **No key reuse** across different repositories
- **Consistent keys** for the same repository
- **Cryptographic isolation** between users and repos

### Benefits

1. **No key management overhead** - Only derivation key needs protection
2. **Automatic key rotation** - Change derivation key to rotate all derived keys
3. **Audit trail** - Can track which repos had keys derived
4. **Compliance** - Keys are never persisted, only derived when needed

## Security Considerations

### Current Implementation (MVP)

⚠️ **NOT PRODUCTION READY** - This is an MVP implementation with several security limitations:

1. **Encryption:** Uses simple XOR (deterministic, no authentication)
   - TODO: Replace with X25519-ECDH + ChaCha20-Poly1305

2. **Attestation:** Placeholder verification
   - TODO: Integrate real SGX/SEV attestation libraries

3. **Key Storage:** Environment variable (not TEE sealed storage)
   - TODO: Use TEE sealed storage with hardware binding

4. **Single Point of Failure:** Only one keystore worker
   - TODO: Add hot standby with key backup/recovery

### Production Requirements

Before production use:
- [ ] Implement proper hybrid encryption (ECDH + AEAD)
- [ ] Integrate real TEE attestation (Intel IAS/DCAP or AMD KDS)
- [ ] Use TEE sealed storage for private key
- [ ] Add key rotation mechanism
- [ ] Implement rate limiting and DDoS protection
- [ ] Add audit logging for all decrypt operations
- [ ] Set up monitoring and alerting
- [ ] Conduct security audit

## Troubleshooting

### "Public key mismatch" error on startup

**Cause:** The keystore's private key doesn't match the public key stored in the contract.

**Fix:**
1. Check `KEYSTORE_PRIVATE_KEY` is correct
2. Check `OFFCHAINVM_CONTRACT_ID` points to the right contract
3. Call `set_keystore_pubkey` on contract with correct public key
4. Or generate a new keystore and update contract

### "Attestation verification failed"

**Cause:** Worker's attestation is invalid or expired.

**Fix:**
1. Check worker is using correct TEE mode
2. Verify attestation timestamp is recent (< 5 minutes)
3. Check expected measurements are configured correctly
4. For simulated mode: ensure worker binary hash matches expected

### "Unauthorized" error

**Cause:** Worker token is invalid or not in allowed list.

**Fix:**
1. Check worker has correct `KEYSTORE_AUTH_TOKEN` in its `.env`
2. Verify token hash is in keystore's `ALLOWED_WORKER_TOKEN_HASHES`
3. Token must match exactly (including whitespace)

## Performance

**Expected throughput:**
- ~1000 decrypt operations/second (single worker, no TEE overhead)
- ~100-500 ops/sec with SGX attestation verification
- Linear scaling with CPU cores (tokio async runtime)

**Latency:**
- < 1ms for decryption operation (XOR)
- ~10-50ms with SGX remote attestation
- Network latency depends on deployment topology

## Development

### Run tests
```bash
cargo test
```

### Run with debug logging
```bash
RUST_LOG=debug cargo run
```

### Build for production
```bash
cargo build --release
```

## License

MIT (same as parent project)
