#!/usr/bin/env python3
"""
Upload WASM file to FastFS using Borsh serialization.

Usage: python3 upload_wasm_fastfs.py <wasm_file> [env_file]

Example:
  python3 upload_wasm_fastfs.py ../wasi-examples/test-storage-ark/target/wasm32-wasip2/release/test-storage-ark.wasm
"""

import sys
import os
import hashlib
import subprocess
import struct

def load_env(env_file):
    """Load environment variables from file."""
    env = {}
    with open(env_file) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#') and '=' in line:
                key, _, value = line.partition('=')
                env[key.strip()] = value.strip()
    return env

def borsh_string(s):
    """Borsh serialize a string (u32 length + bytes)."""
    encoded = s.encode('utf-8')
    return struct.pack('<I', len(encoded)) + encoded

def borsh_bytes(b):
    """Borsh serialize bytes (u32 length + bytes)."""
    return struct.pack('<I', len(b)) + b

def create_fastfs_payload(relative_path, mime_type, content):
    """Create Borsh-serialized FastFS payload."""
    # FastfsData::Simple variant (index 0)
    # SimpleFastfs { relative_path: String, content: Option<FastfsFileContent> }
    # FastfsFileContent { mime_type: String, content: Vec<u8> }

    payload = b''
    # Enum variant index (0 = Simple)
    payload += struct.pack('<B', 0)
    # relative_path: String
    payload += borsh_string(relative_path)
    # content: Option<FastfsFileContent> = Some (1)
    payload += struct.pack('<B', 1)
    # mime_type: String
    payload += borsh_string(mime_type)
    # content: Vec<u8>
    payload += borsh_bytes(content)

    return payload

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 upload_wasm_fastfs.py <wasm_file> [env_file]")
        print()
        print("Example:")
        print("  python3 upload_wasm_fastfs.py ../wasi-examples/test-storage-ark/target/wasm32-wasip2/release/test-storage-ark.wasm")
        sys.exit(1)

    wasm_file = sys.argv[1]
    script_dir = os.path.dirname(os.path.abspath(__file__))
    env_file = sys.argv[2] if len(sys.argv) > 2 else os.path.join(script_dir, '../worker/.env.dev.worker1')

    if not os.path.exists(wasm_file):
        print(f"Error: WASM file not found: {wasm_file}")
        sys.exit(1)

    if not os.path.exists(env_file):
        print(f"Error: Env file not found: {env_file}")
        sys.exit(1)

    # Load env
    env = load_env(env_file)
    receiver = env.get('FASTFS_RECEIVER')
    sender_account = env.get('FASTFS_SENDER_ACCOUNT_ID')
    sender_key = env.get('FASTFS_SENDER_PRIVATE_KEY')

    if not all([receiver, sender_account, sender_key]):
        print("Error: Missing FASTFS_* vars in env file")
        print("Required: FASTFS_RECEIVER, FASTFS_SENDER_ACCOUNT_ID, FASTFS_SENDER_PRIVATE_KEY")
        sys.exit(1)

    # Read WASM file
    with open(wasm_file, 'rb') as f:
        wasm_content = f.read()

    # Calculate SHA256
    wasm_hash = hashlib.sha256(wasm_content).hexdigest()
    relative_path = f"{wasm_hash}.wasm"

    print("Uploading WASM to FastFS...")
    print(f"  File: {wasm_file}")
    print(f"  Size: {len(wasm_content)} bytes")
    print(f"  SHA256: {wasm_hash}")
    print(f"  Sender: {sender_account}")
    print(f"  Receiver: {receiver}")
    print()

    # Create Borsh payload
    payload = create_fastfs_payload(relative_path, "application/wasm", wasm_content)

    # Write payload to temp file
    import tempfile
    with tempfile.NamedTemporaryFile(delete=False, suffix='.bin') as tmp:
        tmp.write(payload)
        payload_file = tmp.name

    try:
        # Call near CLI
        # Format: near contract call-function as-transaction <receiver> <method> file-args <file> prepaid-gas '...' attached-deposit '...' sign-as <account> network-config testnet sign-with-plaintext-private-key <key> send
        cmd = [
            'near', 'contract', 'call-function', 'as-transaction',
            receiver, '__fastdata_fastfs',
            'file-args', payload_file,
            'prepaid-gas', '300 Tgas',
            'attached-deposit', '0 NEAR',
            'sign-as', sender_account,
            'network-config', 'testnet',
            'sign-with-plaintext-private-key', sender_key,
            'send'
        ]

        print("Running:", ' '.join(cmd[:10]) + ' ...')
        result = subprocess.run(cmd, capture_output=True, text=True)

        # Print CLI output for debugging
        if result.stdout:
            print("\n--- NEAR CLI stdout ---")
            print(result.stdout)
        if result.stderr:
            print("\n--- NEAR CLI stderr ---")
            print(result.stderr)

        # Try to extract transaction hash from output
        tx_hash = None
        for line in (result.stdout or '').split('\n') + (result.stderr or '').split('\n'):
            # near-cli-rs outputs: "Transaction ID: <hash>"
            if 'Transaction ID:' in line:
                tx_hash = line.split('Transaction ID:')[1].strip().split()[0]
                break
            # Alternative format: "Transaction sent: <hash>"
            if 'Transaction sent:' in line:
                tx_hash = line.split(':')[1].strip().split()[0]
                break

        # FastFS doesn't have actual contract code - the indexer picks up the transaction
        # So "CodeDoesNotExist" error is expected and means success
        # But other errors should be reported

        if result.returncode != 0:
            # Check if it's the expected "CodeDoesNotExist" error
            stderr = result.stderr or ''
            stdout = result.stdout or ''
            output = stderr + stdout

            if 'CodeDoesNotExist' in output:
                # This is expected - FastFS has no contract, indexer picks up the tx
                pass
            else:
                print("ERROR: Transaction failed!")
                print()
                if stdout:
                    print("STDOUT:")
                    print(stdout)
                if stderr:
                    print("STDERR:")
                    print(stderr)
                print()
                print("Hint: If the payload is too large, NEAR has a ~4MB transaction limit.")
                print(f"      Your payload size: {len(payload)} bytes ({len(payload) / 1024 / 1024:.2f} MB)")
                sys.exit(1)

        print()
        if tx_hash:
            print(f"Transaction hash: {tx_hash}")
            print(f"Explorer: https://testnet.nearblocks.io/txns/{tx_hash}")
        elif result.returncode != 0:
            print(f"Warning: near CLI returned exit code {result.returncode}")

        print()
        print("Upload complete!")
        print()
        print(f"FastFS URL: https://{sender_account}.fastfs.io/{receiver}/{relative_path}")
        print()
        print(f"code_source: {{ \"fastfs\": \"{wasm_hash}\" }}")

    finally:
        os.unlink(payload_file)

if __name__ == '__main__':
    main()
