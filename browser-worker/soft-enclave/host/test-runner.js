import { EnclaveClient } from './enclave-client.js';

const q = (id) => document.getElementById(id);
const results = q('results');
const logEl = q('log');
const summary = q('summary');

function addLine(s){ logEl.textContent += s + "\n"; }
function addResult(name, ok, detail=""){
  const li = document.createElement('li');
  li.innerHTML = `${ok ? '✅' : '❌'} <b>${name}</b>${detail ? ' — ' + detail : ''}`;
  li.style.color = ok ? '#22c55e' : '#ef4444';
  results.appendChild(li);
}

async function testSOPBarrier(client){
  try {
    let threw = false;
    try { client.iframe.contentWindow.document; } catch(e){ threw = true; }
    if (!threw) throw new Error('access did not throw');
    addResult('SOP barrier', true);
  } catch (e) { addResult('SOP barrier', false, String(e.message || e)); }
}

async function testCiphertextOnlyEgress(client){
  try {
    await client.send('test_egress_violation', {}, 'op=test_egress_violation');
    addResult('Ciphertext-only egress', false, 'unexpected success');
  } catch (e) {
    if (String(e.message).toLowerCase().includes('plaintext egress blocked')) {
      addResult('Ciphertext-only egress', true);
    } else {
      addResult('Ciphertext-only egress', false, String(e.message));
    }
  }
}

async function testNonceReplay(client){
  try {
    await client.send('evalQuickJS', { code: '1+2' }, 'op=evalQuickJS');
    const payload = client.lastPayload();
    client.resendRaw(payload);
    try {
      await client.send('evalQuickJS', { code: '2+3' }, 'op=evalQuickJS');
      addResult('Nonce/replay', false, 'no error observed');
    } catch (e2) {
      if (String(e2.message).includes('replay-detected')) addResult('Nonce/replay', true);
      else addResult('Nonce/replay', false, String(e2.message));
    }
  } catch(e) { addResult('Nonce/replay', false, String(e.message)); }
}

async function testContextBinding(enclaveOrigin){
  try {
    const client = new EnclaveClient(enclaveOrigin);
    await client.boot({ codeHashOverride: 'tampered-hash' });
    try {
      await client.send('evalQuickJS', { code: '40+2' }, 'op=evalQuickJS');
      addResult('Context binding', false, 'unexpected success');
    } catch (e) { addResult('Context binding', true); }
  } catch (e) { addResult('Context binding', false, String(e.message)); }
}

async function runAll() {
  results.textContent = '';
  logEl.textContent = '';
  summary.textContent = '';

  const enclaveOrigin = q('enclaveOrigin').value.trim();
  addLine(`Booting enclave at ${enclaveOrigin} ...`);
  const client = new EnclaveClient(enclaveOrigin);
  await client.boot();
  addLine('Enclave ready; session established.');

  await testSOPBarrier(client);
  await testCiphertextOnlyEgress(client);
  await testNonceReplay(client);
  await testContextBinding(enclaveOrigin);

  const items = Array.from(results.querySelectorAll('li'));
  const ok = items.filter(li => li.textContent.trim().startsWith('✅')).length;
  const total = items.length;
  summary.textContent = `Passed ${ok}/${total}`;
}

document.getElementById('run').addEventListener('click', () => {
  runAll().catch(err => addLine('Fatal: ' + err.message));
});
