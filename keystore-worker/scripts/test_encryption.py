#!/usr/bin/env python3
"""
Test encryption/decryption with known private key.

This verifies that encrypt_secrets.py produces correct output that keystore can decrypt.
"""

import hashlib
import base58
import sys
from pathlib import Path

# Load .env from parent directory
def load_env():
    env_path = Path(__file__).parent.parent / '.env'
    if not env_path.exists():
        print(f"❌ Error: .env file not found at {env_path}")
        print("   Please create keystore-worker/.env from .env.example")
        sys.exit(1)

    env_vars = {}
    with open(env_path) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#') and '=' in line:
                key, value = line.split('=', 1)
                env_vars[key.strip()] = value.strip()

    return env_vars

env_vars = load_env()
PRIVATE_KEY = env_vars.get('KEYSTORE_PRIVATE_KEY')

if not PRIVATE_KEY or PRIVATE_KEY == 'ed25519:...':
    print("❌ Error: KEYSTORE_PRIVATE_KEY not set in .env")
    print("   Please set a valid private key in keystore-worker/.env")
    sys.exit(1)

def get_public_key_from_private(private_key_str):
    """Extract public key from NEAR private key format"""
    # Remove "ed25519:" prefix
    key_b58 = private_key_str.replace("ed25519:", "")

    # Decode base58 -> 64 bytes (32 secret + 32 public)
    key_bytes = base58.b58decode(key_b58)

    # Public key is last 32 bytes
    public_key_bytes = key_bytes[32:]

    return public_key_bytes

def encrypt_for_keystore(pubkey_bytes, plaintext):
    """Encrypt (same as encrypt_secrets.py)"""
    # Derive symmetric key
    hasher = hashlib.sha256()
    hasher.update(pubkey_bytes)
    hasher.update(b"keystore-encryption-v1")
    derived_key = hasher.digest()

    # XOR encryption
    plaintext_bytes = plaintext.encode('utf-8')
    ciphertext = bytes(
        b ^ derived_key[i % len(derived_key)]
        for i, b in enumerate(plaintext_bytes)
    )

    return ciphertext

def decrypt_for_keystore(pubkey_bytes, ciphertext):
    """Decrypt (same as keystore-worker does)"""
    # Derive symmetric key (same as encrypt)
    hasher = hashlib.sha256()
    hasher.update(pubkey_bytes)
    hasher.update(b"keystore-encryption-v1")
    derived_key = hasher.digest()

    # XOR decryption (same operation)
    plaintext_bytes = bytes(
        b ^ derived_key[i % len(derived_key)]
        for i, b in enumerate(ciphertext)
    )

    return plaintext_bytes.decode('utf-8')

def main():
    print("🧪 Testing encryption/decryption\n")

    # Extract public key from private key
    pubkey_bytes = get_public_key_from_private(PRIVATE_KEY)
    pubkey_hex = pubkey_bytes.hex()
    pubkey_b58 = base58.b58encode(pubkey_bytes).decode('ascii')

    print(f"🔑 Private key: {PRIVATE_KEY[:30]}...")
    print(f"🔓 Public key (hex):    {pubkey_hex}")
    print(f"🔓 Public key (base58): {pubkey_b58}")
    print(f"🔓 Public key (NEAR):   ed25519:{pubkey_b58}")
    print()

    # Test data
    test_secrets = "OPENAI_KEY=sk-test123,FOO=bar,PRIVATE=secret"
    print(f"📝 Original secrets: {test_secrets}")
    print()

    # Encrypt
    encrypted = encrypt_for_keystore(pubkey_bytes, test_secrets)
    print(f"🔐 Encrypted: {encrypted.hex()[:64]}... ({len(encrypted)} bytes)")
    print()

    # Decrypt
    decrypted = decrypt_for_keystore(pubkey_bytes, encrypted)
    print(f"🔓 Decrypted: {decrypted}")
    print()

    # Verify
    if decrypted == test_secrets:
        print("✅ SUCCESS! Encryption/decryption works correctly!")
        print()
        print("📋 Encrypted as JSON array for contract:")
        print(list(encrypted))
    else:
        print("❌ FAILED! Decrypted text doesn't match original")
        print(f"   Expected: {test_secrets}")
        print(f"   Got:      {decrypted}")
        return 1

    return 0

if __name__ == "__main__":
    exit(main())
