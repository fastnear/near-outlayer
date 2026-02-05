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

CHUNK_SIZE = 1 << 20  # 1MB

def create_fastfs_partial_payload(relative_path, offset, full_size, content_chunk, mime_type, nonce):
    """Create Borsh-serialized FastFS Partial payload for chunked uploads."""
    # FastfsData::Partial variant (index 1)
    # PartialFastfs { relative_path: String, offset: u32, full_size: u32, mime_type: String, content_chunk: Vec<u8>, nonce: u32 }

    payload = b''
    # Enum variant index (1 = Partial)
    payload += struct.pack('<B', 1)
    # relative_path: String
    payload += borsh_string(relative_path)
    # offset: u32
    payload += struct.pack('<I', offset)
    # full_size: u32
    payload += struct.pack('<I', full_size)
    # mime_type: String
    payload += borsh_string(mime_type)
    # content_chunk: Vec<u8>
    payload += borsh_bytes(content_chunk)
    # nonce: u32
    payload += struct.pack('<I', nonce)

    return payload

def create_fastfs_payloads(relative_path, mime_type, content):
    """Create FastFS payloads as chunks."""
    payloads = []
    nonce = int(__import__('time').time()) - 1769376240
    full_size = len(content)

    for offset in range(0, full_size, CHUNK_SIZE):
        chunk = content[offset:min(offset + CHUNK_SIZE, full_size)]
        payload = create_fastfs_partial_payload(
            relative_path, offset, full_size, chunk, mime_type, nonce
        )
        payloads.append(payload)

    return payloads

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
    # Determine network from receiver
    if receiver.endswith('.near'):
        network = 'mainnet'
    elif receiver.endswith('.testnet'):
        network = 'testnet'
    else:
        print(f"Error: Cannot determine network from receiver '{receiver}'")
        print("Receiver should end with '.near' (mainnet) or '.testnet' (testnet)")
        sys.exit(1)

    print(f"  Sender: {sender_account}")
    print(f"  Receiver: {receiver}")
    print(f"  Network: {network}")
    print()

    # Create Borsh payloads (chunked)
    payloads = create_fastfs_payloads(relative_path, "application/wasm", wasm_content)
    num_chunks = len(payloads)
    print(f"  Chunks: {num_chunks} x {CHUNK_SIZE // 1024}KB max")
    print()

    import tempfile
    tx_hashes = []

    for i, payload in enumerate(payloads):
        # Write payload to temp file
        with tempfile.NamedTemporaryFile(delete=False, suffix='.bin') as tmp:
            tmp.write(payload)
            payload_file = tmp.name

        try:
            # Call near CLI
            cmd = [
                'near', 'contract', 'call-function', 'as-transaction',
                receiver, '__fastdata_fastfs',
                'file-args', payload_file,
                'prepaid-gas', '300 Tgas',
                'attached-deposit', '0 NEAR',
                'sign-as', sender_account,
                'network-config', network,
                'sign-with-plaintext-private-key', sender_key,
                'send'
            ]

            print(f"Uploading chunk {i + 1}/{num_chunks}...")
            result = subprocess.run(cmd, capture_output=True, text=True)

            # Try to extract transaction hash from output
            tx_hash = None
            for line in (result.stdout or '').split('\n') + (result.stderr or '').split('\n'):
                if 'Transaction ID:' in line:
                    tx_hash = line.split('Transaction ID:')[1].strip().split()[0]
                    break
                if 'Transaction sent:' in line:
                    tx_hash = line.split(':')[1].strip().split()[0]
                    break

            # FastFS doesn't have actual contract code - the indexer picks up the transaction
            # So "CodeDoesNotExist" error is expected and means success
            if result.returncode != 0:
                stderr = result.stderr or ''
                stdout = result.stdout or ''
                output = stderr + stdout

                if 'CodeDoesNotExist' not in output:
                    print(f"ERROR: Chunk {i + 1} failed!")
                    if stdout:
                        print("STDOUT:", stdout)
                    if stderr:
                        print("STDERR:", stderr)
                    sys.exit(1)

            if tx_hash:
                tx_hashes.append(tx_hash)
                print(f"  tx: {tx_hash}")

        finally:
            os.unlink(payload_file)

    print()
    print("Upload complete!")
    print()
    print(f"FastFS URL: https://{sender_account}.fastfs.io/{receiver}/{relative_path}")
    print()
    print(f"code_source: {{ \"fastfs\": \"{wasm_hash}\" }}")

if __name__ == '__main__':
    main()
