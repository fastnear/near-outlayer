#!/usr/bin/env python3
"""
Simple script to encrypt secrets for NEAR Offshore contract.

Usage:
    ./encrypt_secrets.py "OPENAI_KEY=sk-...,FOO=bar"
    ./encrypt_secrets.py "OPENAI_KEY=sk-test" --keystore http://localhost:8081

Output: JSON array for contract's encrypted_secrets parameter
"""

import sys
import json
import requests
import hashlib

def get_pubkey(keystore_url="http://localhost:8081"):
    """Get public key from keystore worker"""
    try:
        resp = requests.get(f"{keystore_url}/pubkey", timeout=5)
        resp.raise_for_status()
        data = resp.json()
        return data["public_key_hex"]
    except Exception as e:
        print(f"Error: Failed to get public key from {keystore_url}/pubkey", file=sys.stderr)
        print(f"  {e}", file=sys.stderr)
        print(f"  Make sure keystore-worker is running on {keystore_url}", file=sys.stderr)
        sys.exit(1)

def encrypt_for_keystore(pubkey_hex, plaintext):
    """
    Encrypt plaintext using keystore's public key.

    NOTE: This uses the SAME simple XOR encryption as keystore-worker (MVP).
    For production, this should be replaced with proper hybrid encryption:
    - X25519 ECDH for key exchange
    - ChaCha20-Poly1305 for authenticated encryption
    """
    # Derive symmetric key from public key (same as keystore does)
    key_material = bytes.fromhex(pubkey_hex)
    hasher = hashlib.sha256()
    hasher.update(key_material)
    hasher.update(b"keystore-encryption-v1")
    derived_key = hasher.digest()

    # XOR encryption (matches keystore decrypt)
    plaintext_bytes = plaintext.encode('utf-8')
    ciphertext = bytes(
        b ^ derived_key[i % len(derived_key)]
        for i, b in enumerate(plaintext_bytes)
    )

    return ciphertext

def main():
    if len(sys.argv) < 2:
        print("Usage: ./encrypt_secrets.py '{\"KEY1\":\"value1\",\"KEY2\":\"value2\"}' [--keystore URL]")
        print()
        print("Examples:")
        print("  ./encrypt_secrets.py '{\"OPENAI_KEY\":\"sk-...\"}'")
        print("  ./encrypt_secrets.py '{\"OPENAI_KEY\":\"sk-...\",\"FOO\":\"bar\"}'")
        print("  ./encrypt_secrets.py '{\"SECRET\":\"test\"}' --keystore http://localhost:8081")
        sys.exit(1)

    secrets_json = sys.argv[1]
    keystore_url = "http://localhost:8081"

    # Validate JSON format
    try:
        parsed = json.loads(secrets_json)
        if not isinstance(parsed, dict):
            print("Error: Secrets must be a JSON object, e.g. {\"KEY\":\"value\"}", file=sys.stderr)
            sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON format: {e}", file=sys.stderr)
        sys.exit(1)

    # Parse optional --keystore argument
    if len(sys.argv) > 2 and sys.argv[2] == "--keystore":
        if len(sys.argv) < 4:
            print("Error: --keystore requires URL argument", file=sys.stderr)
            sys.exit(1)
        keystore_url = sys.argv[3]

    # Get public key from keystore
    print(f"🔑 Fetching public key from {keystore_url}...", file=sys.stderr)
    pubkey_hex = get_pubkey(keystore_url)
    print(f"✅ Public key: {pubkey_hex[:16]}...", file=sys.stderr)

    # Encrypt secrets
    print(f"🔐 Encrypting secrets: {secrets_json[:50]}...", file=sys.stderr)
    encrypted = encrypt_for_keystore(pubkey_hex, secrets_json)
    print(f"✅ Encrypted: {len(encrypted)} bytes", file=sys.stderr)

    # Convert to JSON array for contract
    encrypted_array = list(encrypted)

    # Output JSON (suitable for contract call)
    print()
    print("📋 Copy this for contract call:")
    print()
    print(json.dumps(encrypted_array))
    print()
    print("📝 Or use in near-cli command:")
    print()
    print(f'near call offchainvm.testnet request_execution \\')
    print(f'  \'{{"code_source": {{"repo":"...", "commit":"...", "build_target":"wasm32-wasi"}}, \\')
    print(f'    "resource_limits": {{"max_instructions": 1000000000, "max_memory_mb": 128, "max_execution_seconds": 60}}, \\')
    print(f'    "input_data": "{{}}", \\')
    print(f'    "encrypted_secrets": {json.dumps(encrypted_array)}}}\' \\')
    print(f'  --accountId user.testnet --deposit 0.1')

if __name__ == "__main__":
    main()
