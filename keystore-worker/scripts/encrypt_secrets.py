#!/usr/bin/env python3
"""
Script to encrypt secrets for NEAR OutLayer repo-based secrets system.

Usage:
    ./encrypt_secrets.py --repo alice/project --owner alice.near --profile default '{"OPENAI_KEY":"sk-..."}'
    ./encrypt_secrets.py --repo alice/project --owner alice.near --branch main --profile prod '{"API_KEY":"secret"}'

Output: Encrypted secrets as base64 string for contract's store_secrets method
"""

import sys
import json
import requests
import hashlib
import argparse
import base64

def get_pubkey_from_coordinator(coordinator_url, repo, owner, branch=None):
    """Get public key from coordinator (which proxies to keystore)"""
    try:
        params = {
            "repo": repo,
            "owner": owner
        }
        if branch:
            params["branch"] = branch

        resp = requests.get(f"{coordinator_url}/secrets/pubkey", params=params, timeout=5)
        resp.raise_for_status()
        data = resp.json()

        print(f"🔑 Seed: {data.get('seed', 'N/A')}", file=sys.stderr)
        return data["pubkey"]
    except Exception as e:
        print(f"Error: Failed to get public key from {coordinator_url}", file=sys.stderr)
        print(f"  {e}", file=sys.stderr)
        print(f"  Make sure coordinator is running on {coordinator_url}", file=sys.stderr)
        sys.exit(1)

def get_pubkey_from_keystore(keystore_url, repo, owner, branch=None):
    """Get public key directly from keystore (bypasses coordinator)"""
    try:
        # Build seed: repo:owner[:branch]
        # Normalize repo (remove github.com if present)
        if repo.startswith("https://github.com/"):
            repo = repo.replace("https://github.com/", "github.com/")
        elif repo.startswith("git@github.com:"):
            repo = "github.com/" + repo.replace("git@github.com:", "").replace(".git", "")
        elif not repo.startswith("github.com/"):
            repo = f"github.com/{repo}"

        seed = f"{repo}:{owner}"
        if branch:
            seed = f"{seed}:{branch}"

        print(f"🔑 Seed: {seed}", file=sys.stderr)

        resp = requests.get(f"{keystore_url}/pubkey", params={"seed": seed}, timeout=5)
        resp.raise_for_status()
        data = resp.json()
        return data["public_key_hex"]
    except Exception as e:
        print(f"Error: Failed to get public key from {keystore_url}", file=sys.stderr)
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
    parser = argparse.ArgumentParser(
        description="Encrypt secrets for NEAR OutLayer repo-based secrets",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Encrypt secrets for alice/project owned by alice.near:
  ./encrypt_secrets.py --repo alice/project --owner alice.near --profile default '{"OPENAI_KEY":"sk-..."}'

  # With specific branch:
  ./encrypt_secrets.py --repo alice/project --owner alice.near --branch main --profile prod '{"API_KEY":"secret"}'

  # Use keystore directly (bypass coordinator):
  ./encrypt_secrets.py --repo alice/project --owner alice.near --profile default '{"SECRET":"value"}' --keystore http://localhost:8081
        """
    )
    parser.add_argument("--repo", required=True, help="GitHub repository (e.g., alice/project or https://github.com/alice/project)")
    parser.add_argument("--owner", required=True, help="NEAR account ID that will own these secrets")
    parser.add_argument("--branch", help="Optional branch name (omit for all branches)")
    parser.add_argument("--profile", required=True, help="Profile name (e.g., default, prod, staging)")
    parser.add_argument("--coordinator", default="http://localhost:8080", help="Coordinator URL (default: http://localhost:8080)")
    parser.add_argument("--keystore", help="Keystore URL (bypasses coordinator if set)")
    parser.add_argument("secrets_json", help='Secrets as JSON object, e.g. \'{"KEY":"value"}\'')

    args = parser.parse_args()

    # Reserved keywords that should not be overridden by user secrets
    RESERVED_KEYWORDS = [
        "NEAR_SENDER_ID",
        "NEAR_CONTRACT_ID",
        "NEAR_USER_ACCOUNT_ID",
        "NEAR_PAYMENT_YOCTO",
        "NEAR_TRANSACTION_HASH",
        "NEAR_BLOCK_HEIGHT",
        "NEAR_BLOCK_TIMESTAMP",
        "NEAR_MAX_INSTRUCTIONS",
        "NEAR_MAX_MEMORY_MB",
        "NEAR_MAX_EXECUTION_SECONDS",
        "NEAR_REQUEST_ID",
    ]

    # Validate JSON format
    try:
        parsed = json.loads(args.secrets_json)
        if not isinstance(parsed, dict):
            print("Error: Secrets must be a JSON object, e.g. {\"KEY\":\"value\"}", file=sys.stderr)
            sys.exit(1)

        # Check for reserved keywords
        reserved_found = [key for key in parsed.keys() if key in RESERVED_KEYWORDS]
        if reserved_found:
            print(f"❌ Error: Cannot use reserved system keywords as secret keys:", file=sys.stderr)
            for key in reserved_found:
                print(f"  - {key}", file=sys.stderr)
            print(f"\nReserved keywords (automatically set by OutLayer worker):", file=sys.stderr)
            for key in RESERVED_KEYWORDS:
                print(f"  - {key}", file=sys.stderr)
            print(f"\nPlease rename these keys in your secrets.", file=sys.stderr)
            sys.exit(1)

    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON format: {e}", file=sys.stderr)
        sys.exit(1)

    # Get public key (from coordinator or keystore)
    if args.keystore:
        print(f"🔑 Fetching public key from keystore {args.keystore}...", file=sys.stderr)
        pubkey_hex = get_pubkey_from_keystore(args.keystore, args.repo, args.owner, args.branch)
    else:
        print(f"🔑 Fetching public key from coordinator {args.coordinator}...", file=sys.stderr)
        pubkey_hex = get_pubkey_from_coordinator(args.coordinator, args.repo, args.owner, args.branch)

    print(f"✅ Public key: {pubkey_hex[:16]}...", file=sys.stderr)

    # Encrypt secrets
    print(f"🔐 Encrypting secrets: {args.secrets_json[:50]}...", file=sys.stderr)
    encrypted = encrypt_for_keystore(pubkey_hex, args.secrets_json)
    print(f"✅ Encrypted: {len(encrypted)} bytes", file=sys.stderr)

    # Convert to base64 for storage in contract
    encrypted_base64 = base64.b64encode(encrypted).decode('ascii')

    # Output
    print()
    print("📋 Encrypted secrets (base64):")
    print()
    print(encrypted_base64)
    print()
    print("📝 Store in contract with near-cli:")
    print()

    # Normalize repo for display
    repo_display = args.repo
    if repo_display.startswith("https://github.com/"):
        repo_display = repo_display.replace("https://github.com/", "")
    elif repo_display.startswith("github.com/"):
        repo_display = repo_display.replace("github.com/", "")

    branch_arg = f', "branch": "{args.branch}"' if args.branch else ''

    print(f'near call outlayer.testnet store_secrets \\')
    print(f'  \'{{"repo": "{repo_display}", {branch_arg[2:] if branch_arg else ""} \\')
    print(f'    "profile": "{args.profile}", \\')
    print(f'    "encrypted_secrets_base64": "{encrypted_base64}", \\')
    print(f'    "access": {{"AllowAll": {{}}}}}}\' \\')
    print(f'  --accountId {args.owner} \\')
    print(f'  --deposit 0.01')
    print()
    print("🔍 Verify storage:")
    print(f'near view outlayer.testnet get_secrets \'{{"repo": "{repo_display}", {branch_arg[2:] if branch_arg else ""} "profile": "{args.profile}", "owner": "{args.owner}"}}\'')

if __name__ == "__main__":
    main()
