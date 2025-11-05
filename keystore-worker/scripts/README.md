# Keystore Scripts

Helper scripts for repo-based secrets encryption.

## Scripts

### `encrypt_secrets.py`

Encrypts secrets for storing in contract via `store_secrets` method.

**Usage:**
```bash
# Via coordinator (recommended - port 8080)
./encrypt_secrets.py --repo alice/project --owner alice.near --profile default '{"OPENAI_KEY":"sk-..."}'

# With branch
./encrypt_secrets.py --repo alice/project --owner alice.near --branch main --profile prod '{"API_KEY":"secret"}'

# Direct to keystore (for testing only - port 8081)
./encrypt_secrets.py --repo alice/project --owner alice.near --profile default '{"KEY":"value"}' --keystore http://localhost:8081
```

**Output:**
```
🔑 Seed: github.com/alice/project:alice.near
📦 Encrypted secrets (base64): aGVsbG8gd29ybGQ...
```

Copy the base64 string and use in contract's `store_secrets` method.

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

## Example: Full Flow with Repo-Based Secrets

**1. Start services:**
```bash
# Coordinator (port 8080)
cd coordinator
cargo run

# Keystore (port 8081)
cd keystore-worker
docker-compose up -d
```

**2. Encrypt secrets:**
```bash
cd keystore-worker/scripts
./encrypt_secrets.py --repo alice/myproject --owner alice.testnet --profile default '{"OPENAI_KEY":"sk-test123"}'

# Output:
# 🔑 Seed: github.com/alice/myproject:alice.testnet
# 📦 Encrypted secrets (base64): YWJjZGVmZ2hpams...
```

**3. Store secrets in contract:**
```bash
near call outlayer.testnet store_secrets \
  '{
    "repo": "github.com/alice/myproject",
    "branch": null,
    "profile": "default",
    "encrypted_secrets_base64": "YWJjZGVmZ2hpams...",
    "access": "AllowAll"
  }' \
  --accountId alice.testnet \
  --deposit 0.01
```

**4. Request execution with secrets:**
```bash
near call outlayer.testnet request_execution \
  '{
    "code_source": {
      "repo": "https://github.com/alice/myproject",
      "commit": "main",
      "build_target": "wasm32-wasip1"
    },
    "secrets_ref": {
      "profile": "default",
      "account_id": "alice.testnet"
    },
    "resource_limits": {
      "max_instructions": 1000000000,
      "max_memory_mb": 128,
      "max_execution_seconds": 60
    },
    "input_data": "{}"
  }' \
  --accountId user.testnet \
  --deposit 0.1
```

**5. Worker will:**
- Fetch encrypted secrets from contract (repo + branch + profile + owner)
- Decrypt via keystore with access control validation
- Inject secrets into WASM environment variables
- Execute WASM with `std::env::var("OPENAI_KEY")`

## Requirements

Python 3.6+ with:
```bash
pip install requests base64
```

## Security Notes

⚠️ **MVP Implementation**: Current encryption uses simple XOR (for testing).

For production, replace with:
- X25519 ECDH for key agreement
- ChaCha20-Poly1305 for authenticated encryption

## See Also

- Use Dashboard UI at http://localhost:3000/secrets for easier secret management
- Dashboard provides graphical interface for creating/editing/deleting secrets
