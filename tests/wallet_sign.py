#!/usr/bin/env python3
"""
Ed25519 signing helper for wallet integration tests.

Usage:
  python3 wallet_sign.py generate
    -> prints: PRIVATE_KEY_HEX WALLET_ID

  python3 wallet_sign.py sign <private_key_hex> <timestamp> <payload>
    -> prints: signature_hex

Requires: pip install pynacl
"""

import sys
import hashlib

try:
    from nacl.signing import SigningKey
except ImportError:
    print("ERROR: pynacl not installed. Run: pip install pynacl", file=sys.stderr)
    sys.exit(1)


def generate():
    """Generate a new Ed25519 keypair."""
    sk = SigningKey.generate()
    private_hex = sk.encode().hex()
    public_hex = sk.verify_key.encode().hex()
    wallet_id = f"ed25519:{public_hex}"
    print(f"{private_hex} {wallet_id}")


def from_private(private_key_hex: str):
    """Derive wallet_id from existing private key."""
    sk = SigningKey(bytes.fromhex(private_key_hex))
    public_hex = sk.verify_key.encode().hex()
    wallet_id = f"ed25519:{public_hex}"
    print(f"{private_key_hex} {wallet_id}")


def sign(private_key_hex: str, timestamp: str, payload: str):
    """Sign timestamp:payload with the private key."""
    sk = SigningKey(bytes.fromhex(private_key_hex))
    message = f"{timestamp}:{payload}".encode()
    signed = sk.sign(message)
    print(signed.signature.hex())


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} generate|sign|from_private", file=sys.stderr)
        sys.exit(1)

    cmd = sys.argv[1]

    if cmd == "generate":
        generate()
    elif cmd == "from_private":
        if len(sys.argv) < 3:
            print(f"Usage: {sys.argv[0]} from_private <privkey_hex>", file=sys.stderr)
            sys.exit(1)
        from_private(sys.argv[2])
    elif cmd == "sign":
        if len(sys.argv) < 5:
            print(f"Usage: {sys.argv[0]} sign <privkey_hex> <timestamp> <payload>", file=sys.stderr)
            sys.exit(1)
        sign(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        print(f"Unknown command: {cmd}", file=sys.stderr)
        sys.exit(1)
