import {
  genECDH, exportRaw, importRaw, deriveSession,
  seal, open, serializePayload, deserializePayload
} from './crypto-protocol.js';
import { createQuickJS } from './quickjs-runtime.js';

const EXPECTED_PARENT = new URL(document.referrer || 'http://localhost:8080').origin;

let port = null;
let session = null;
let seqSend = 1;

const seen = new Set(); const fifo = []; const MAX = 4096;
function rememberIV(iv) {
  const k = Array.from(iv).map(b=>b.toString(16).padStart(2,'0')).join('');
  if (seen.has(k)) return false;
  seen.add(k); fifo.push(k);
  if (fifo.length > MAX) { const old = fifo.shift(); seen.delete(old); }
  return true;
}

function postToParent(type, data) {
  window.parent.postMessage({ type, ...data }, EXPECTED_PARENT);
}

function guardedPost(msg) {
  port.postMessage(msg);
}

(async function main(){
  await new Promise(resolve => {
    window.addEventListener('message', (ev) => {
      if (ev.origin !== EXPECTED_PARENT) return;
      if (ev.data?.type === 'connect' && ev.ports?.[0]) {
        port = ev.ports[0];
        port.start();
        resolve();
      }
    }, { once:true });
  });

  const qjs = await createQuickJS();

  const keys = await genECDH();
  const enclavePub = await exportRaw(keys.publicKey);
  const codeHash = 'soft-enclave-demo-v1';

  postToParent('enclave-ready', { enclavePubKey: Array.from(enclavePub), codeHash });

  port.onmessage = async (evt) => {
    const msg = evt.data || {};
    try {
      if (msg.type === 'host-hello') {
        const hostPub = await importRaw(new Uint8Array(msg.hostPubKey));
        session = await deriveSession(keys.privateKey, hostPub, {
          hostOrigin: EXPECTED_PARENT,
          enclaveOrigin: location.origin,
          codeHash
        });
        return;
      }

      if (msg.type === 'ciphertext-call') {
        if (!session) throw new Error('session-not-established');
        const payload = deserializePayload(msg.payload);

        if (!rememberIV(payload.iv)) throw new Error('replay-detected');

        const call = await open(session.aeadKey, payload, 'op=evalQuickJS');
        if (call.op === 'evalQuickJS') {
          const out = qjs.evalQuickJS(call.code);
          const ct = await seal(session.aeadKey, session.baseIV, seqSend++, out, 'op=evalQuickJS:result');
          guardedPost({ type:'ciphertext-result', payload: serializePayload(ct), seq: seqSend - 1 });
          return;
        }
        if (call.op === 'test_egress_violation') {
          try {
            port.postMessage({ type:'not-allowed-plaintext', value: 1 });
            guardedPost({ type:'error', error:'egress-guard-did-not-trigger' });
          } catch(e) {
            guardedPost({ type:'error', error:String(e) });
          }
          return;
        }

        throw new Error('unsupported-op');
      }

      throw new Error('unsupported-message');
    } catch (e) {
      guardedPost({ type:'error', error:String(e) });
    }
  };
})();
