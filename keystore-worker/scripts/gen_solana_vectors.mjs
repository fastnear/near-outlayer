// Generates src/solana_vectors.json — @solana/web3.js reference vectors
// pinning keystore-worker/src/solana.rs (tx-message guard) and the ed25519
// signing path (crypto.rs::sign) byte-for-byte against the ecosystem tooling.
//
// Usage:
//   npm i @solana/web3.js@1 tweetnacl bs58   (any scratch dir)
//   node gen_solana_vectors.mjs [output-path]
//
// NOTE: pin web3.js to the 1.x line. The 2.x rewrite (renamed to
// @solana/kit) has an incompatible API and this script will not run on it.
//
// Deliberately deterministic (fixed master, fixed blockhash, no randomness):
// re-running must reproduce the committed file exactly.
//
// SECURITY NOTE: the test master secret and every key derived from it are
// PUBLIC test fixtures. They hold no funds and never will — do not fund them.

import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  AddressLookupTableAccount,
} from '@solana/web3.js';
import nacl from 'tweetnacl';
import bs58 from 'bs58';
import { createHash, createHmac } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Mirrors crypto.rs: key = HMAC-SHA256(master, seed), fed to ed25519 as the
// 32-byte seed (ed25519_dalek::SigningKey::from_bytes == Keypair.fromSeed).
const TEST_MASTER_HEX =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff';
const WALLET_ID = 'test-wallet-1';
const SEED = `wallet:${WALLET_ID}:solana`; // api.rs::wallet_seed(wallet_id, "solana")

const hmacDerive = (seed) =>
  createHmac('sha256', Buffer.from(TEST_MASTER_HEX, 'hex')).update(seed).digest();
const sha256 = (s) => createHash('sha256').update(s).digest();

const wallet = Keypair.fromSeed(hmacDerive(SEED));
const dest = Keypair.fromSeed(hmacDerive('vector:dest')); // throwaway recipient
const lookupTableKey = new PublicKey(sha256('vector:lookup-table-account'));
const RECENT_BLOCKHASH = bs58.encode(sha256('vector:recent-blockhash'));

const b64 = (bytes) => Buffer.from(bytes).toString('base64');
const signDetached = (bytes) =>
  nacl.sign.detached(Uint8Array.from(bytes), wallet.secretKey);

const transferIx = SystemProgram.transfer({
  fromPubkey: wallet.publicKey,
  toPubkey: dest.publicKey,
  lamports: 1_000_000,
});
const memoIx = new TransactionInstruction({
  programId: new PublicKey('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr'),
  keys: [],
  data: Buffer.from('outlayer solana vectors', 'utf8'),
});

// --- legacy messages ------------------------------------------------------
function legacyTx(instructions) {
  const tx = new Transaction({
    recentBlockhash: RECENT_BLOCKHASH,
    feePayer: wallet.publicKey,
  });
  tx.add(...instructions);
  return tx;
}
const legacyTransfer = legacyTx([transferIx]);
const legacyTransferMsg = legacyTransfer.serializeMessage();
const legacyTwo = legacyTx([memoIx, transferIx]);
const legacyTwoMsg = legacyTwo.serializeMessage();

// --- v0 messages ----------------------------------------------------------
const v0Msg = new TransactionMessage({
  payerKey: wallet.publicKey,
  recentBlockhash: RECENT_BLOCKHASH,
  instructions: [transferIx],
}).compileToV0Message();
const v0MsgBytes = v0Msg.serialize();

// v0 with an address-table lookup: route the transfer recipient through a
// fabricated lookup table so the message carries a non-empty lookups section.
const lookupTable = new AddressLookupTableAccount({
  key: lookupTableKey,
  state: {
    deactivationSlot: BigInt('0xffffffffffffffff'),
    lastExtendedSlot: 0,
    lastExtendedSlotStartIndex: 0,
    addresses: [dest.publicKey],
  },
});
const v0LookupMsg = new TransactionMessage({
  payerKey: wallet.publicKey,
  recentBlockhash: RECENT_BLOCKHASH,
  instructions: [transferIx],
}).compileToV0Message([lookupTable]);
const v0LookupMsgBytes = v0LookupMsg.serialize();
if (v0LookupMsg.addressTableLookups.length !== 1) {
  throw new Error('lookup table was not used — vector would be mislabeled');
}

// --- signing: sign with web3.js/nacl, record expected signatures ----------
function signedLegacy(tx, msgBytes) {
  tx.sign(wallet);
  const wire = tx.serialize();
  if (!tx.verifySignatures()) throw new Error('legacy self-verify failed');
  const sig = tx.signature;
  if (Buffer.compare(sig, Buffer.from(signDetached(msgBytes))) !== 0) {
    throw new Error('web3.js and nacl disagree on the legacy signature');
  }
  return { sig, wire };
}
function signedV0(msg, msgBytes) {
  const vtx = new VersionedTransaction(msg);
  vtx.sign([wallet]);
  const sig = vtx.signatures[0];
  if (Buffer.compare(Buffer.from(sig), Buffer.from(signDetached(msgBytes))) !== 0) {
    throw new Error('web3.js and nacl disagree on the v0 signature');
  }
  return { sig, wire: vtx.serialize() };
}

const legacySigned = signedLegacy(legacyTransfer, legacyTransferMsg);
const v0Signed = signedV0(v0Msg, v0MsgBytes);

const siwsText =
  'example.com wants you to sign in with your Solana account:\n' +
  `${wallet.publicKey.toBase58()}\n\nSign in to Example\n\n` +
  'URI: https://example.com\nVersion: 1\nChain ID: mainnet\n' +
  'Nonce: deadbeef01\nIssued At: 2026-01-01T00:00:00Z';
const binaryMessage = sha256('vector:opaque-binary-message');

// deterministic pseudo-random blob (sha256 chain, 96 bytes)
const randomBlob = Buffer.concat([sha256('blob:1'), sha256('blob:2'), sha256('blob:3')]);

const vectors = {
  _comment:
    'Generated by scripts/gen_solana_vectors.mjs — do not edit by hand. ' +
    'The master/keys here are PUBLIC test fixtures holding no funds; never fund them.',
  signing: {
    test_master_hex: TEST_MASTER_HEX,
    wallet_id: WALLET_ID,
    seed: SEED,
    address_base58: wallet.publicKey.toBase58(),
    cases: [
      {
        label: 'message_siws_utf8',
        kind: 'message',
        payload_base64: b64(Buffer.from(siwsText, 'utf8')),
        signature_base58: bs58.encode(signDetached(Buffer.from(siwsText, 'utf8'))),
      },
      {
        label: 'message_opaque_binary',
        kind: 'message',
        payload_base64: b64(binaryMessage),
        signature_base58: bs58.encode(signDetached(binaryMessage)),
      },
      {
        label: 'tx_legacy_transfer',
        kind: 'transaction',
        payload_base64: b64(legacyTransferMsg),
        signature_base58: bs58.encode(legacySigned.sig),
        signed_tx_base64: b64(legacySigned.wire),
      },
      {
        label: 'tx_v0_transfer',
        kind: 'transaction',
        payload_base64: b64(v0MsgBytes),
        signature_base58: bs58.encode(v0Signed.sig),
        signed_tx_base64: b64(v0Signed.wire),
      },
    ],
  },
  guard_cases: [
    { label: 'legacy_transfer', bytes_base64: b64(legacyTransferMsg), is_tx_message: true },
    { label: 'legacy_two_instructions', bytes_base64: b64(legacyTwoMsg), is_tx_message: true },
    { label: 'v0_transfer', bytes_base64: b64(v0MsgBytes), is_tx_message: true },
    { label: 'v0_with_lookup_table', bytes_base64: b64(v0LookupMsgBytes), is_tx_message: true },
    { label: 'siws_text', bytes_base64: b64(Buffer.from(siwsText, 'utf8')), is_tx_message: false },
    { label: 'opaque_binary_32b', bytes_base64: b64(binaryMessage), is_tx_message: false },
    { label: 'random_blob_96b', bytes_base64: b64(randomBlob), is_tx_message: false },
    {
      label: 'legacy_truncated',
      bytes_base64: b64(legacyTransferMsg.slice(0, legacyTransferMsg.length - 1)),
      is_tx_message: false,
    },
    {
      label: 'legacy_trailing_byte',
      bytes_base64: b64(Buffer.concat([legacyTransferMsg, Buffer.from([0x00])])),
      is_tx_message: false,
    },
    {
      label: 'json_text',
      bytes_base64: b64(Buffer.from('{"action":"login","nonce":"deadbeef01"}', 'utf8')),
      is_tx_message: false,
    },
  ],
};

const here = dirname(fileURLToPath(import.meta.url));
const out = process.argv[2] ?? join(here, '..', 'src', 'solana_vectors.json');
writeFileSync(out, JSON.stringify(vectors, null, 2) + '\n');
console.log(`wrote ${out}`);
console.log(`wallet address: ${wallet.publicKey.toBase58()}`);
