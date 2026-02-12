# tee-auth

Shared Rust crate for TEE challenge-response authentication. Used by **coordinator** and **keystore-worker** to verify that a worker holds a private key generated inside a TEE and registered on-chain via `register-contract`.

## How It Works

```
Worker (TEE)                    Server (coordinator/keystore)
     │                                    │
     │  1. Request challenge              │
     │ ──────────────────────────────────> │
     │                                    │ generate_challenge()
     │  2. Return 32-byte random hex      │
     │ <────────────────────────────────── │
     │                                    │
     │  sign(challenge, tee_private_key)   │
     │                                    │
     │  3. Submit signature + public_key   │
     │ ──────────────────────────────────> │
     │                                    │ verify_signature()
     │                                    │ check_access_key_on_contract()
     │  4. Session token                  │
     │ <────────────────────────────────── │
```

The server verifies two things:
1. **Signature is valid** — worker possesses the private key
2. **Key exists on NEAR** — key was registered via `register_worker_key()` with TDX attestation proof

## Public API

```rust
/// Generate random 32-byte challenge (hex-encoded, 64 chars)
pub fn generate_challenge() -> String

/// Verify ed25519 signature over challenge
/// Accepts public keys as `ed25519:...` (base58) or raw hex (64 chars)
pub fn verify_signature(
    public_key: &str,
    challenge: &str,
    signature: &str,
) -> Result<(), TeeAuthError>

/// Check if public key exists as access key on a NEAR account
pub fn check_access_key_on_contract(
    client: &reqwest::Client,
    rpc_url: &str,
    account_id: &str,  // e.g., "operator.outlayer.near"
    public_key: &str,
) -> Result<bool, TeeAuthError>

/// Same as above but retries 3 times with 3s delay (handles finality lag)
pub fn check_access_key_with_retry(
    rpc_url: &str,
    account_id: &str,
    public_key: &str,
) -> Result<bool, TeeAuthError>
```

## Dependencies

- `ed25519-dalek` — ed25519 signing/verification
- `bs58` — base58 encoding (NEAR key format)
- `reqwest` — NEAR RPC calls
- `tokio` — async runtime

## Usage

Used as a workspace dependency:

```toml
[dependencies]
tee-auth = { path = "../tee-auth" }
```

Consumers: `coordinator/`, `keystore-worker/`
