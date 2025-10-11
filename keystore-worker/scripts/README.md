# Keystore Scripts

Helper scripts for testing encryption/decryption with keystore-worker.

## Scripts

### `encrypt_secrets.py`

Encrypts secrets for passing to smart contract.

**Usage:**
```bash
# Basic usage (keystore on localhost:8081)
./encrypt_secrets.py "OPENAI_KEY=sk-...,FOO=bar"

# Custom keystore URL
./encrypt_secrets.py "SECRET=test" --keystore http://keystore.example.com:8081
```

**Output:**
```json
[117, 56, 11, 198, 58, 167, ...]
```

Copy this array and use in contract call as `encrypted_secrets` parameter.

### `test_encryption.py`

Tests that encryption/decryption works correctly with a known private key.

**Usage:**
```bash
# Edit PRIVATE_KEY in the script first
./test_encryption.py
```

**Expected output:**
```
✅ SUCCESS! Encryption/decryption works correctly!
```

## Example: Full Flow

**1. Start keystore-worker:**
```bash
cd keystore-worker
KEYSTORE_PRIVATE_KEY=ed25519:5gw3zzg... cargo run
```

**2. Encrypt secrets:**
```bash
cd scripts
./encrypt_secrets.py "OPENAI_KEY=sk-test123"
```

**3. Copy output and call contract:**
```bash
near call offchainvm.testnet request_execution \
  '{
    "code_source": {
      "repo": "https://github.com/user/project",
      "commit": "abc123",
      "build_target": "wasm32-wasi"
    },
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{}",
    "encrypted_secrets": [117, 56, 11, ...]
  }' \
  --accountId user.testnet \
  --deposit 0.1
```

**4. Worker will:**
- Receive task with encrypted_secrets
- Request decryption from keystore
- Execute WASM with decrypted secrets
- Submit result to contract

## Requirements

Python 3.6+ with:
```bash
pip install requests base58
```

## Security Notes

⚠️ **MVP Implementation**: Current encryption uses simple XOR (for testing).

For production, replace with:
- X25519 ECDH for key agreement
- ChaCha20-Poly1305 for authenticated encryption
