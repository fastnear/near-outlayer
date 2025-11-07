import {
  genECDH, exportRaw, importRaw, deriveSession,
  seal, open, serializePayload, deserializePayload
} from './crypto-protocol.js';

export class EnclaveClient {
  constructor(enclaveOrigin) {
    this.enclaveOrigin = enclaveOrigin;
    this.iframe = null;
    this.port = null;
    this.session = null;
    this.seq = 1;
    this._pending = null;
    this._lastSent = null;
    this._onMsg = this._onMsg.bind(this);
  }

  async boot({ codeHashOverride } = {}) {
    this.iframe = document.createElement('iframe');
    this.iframe.src = `${this.enclaveOrigin}/boot.html`;
    this.iframe.style.display = 'none';
    document.body.appendChild(this.iframe);

    const { port1, port2 } = new MessageChannel();
    this.port = port1;
    this.port.onmessage = this._onMsg;

    const ready = await new Promise((resolve) => {
      const onReady = (ev) => {
        if (ev.source !== this.iframe.contentWindow) return;
        if (ev.origin !== this.enclaveOrigin) return;
        if (ev.data?.type !== 'enclave-ready') return;
        window.removeEventListener('message', onReady);
        resolve(ev.data);
      };
      window.addEventListener('message', onReady);
      this.iframe.addEventListener('load', () => {
        this.iframe.contentWindow.postMessage({ type: 'connect' }, this.enclaveOrigin, [port2]);
      });
    });

    const enclavePub = await importRaw(new Uint8Array(ready.enclavePubKey));
    const hostKeys = await genECDH();
    const hostPubRaw = await exportRaw(hostKeys.publicKey);

    const ctx = {
      hostOrigin: location.origin,
      enclaveOrigin: this.enclaveOrigin,
      codeHash: codeHashOverride ?? ready.codeHash
    };
    this.session = await deriveSession(hostKeys.privateKey, enclavePub, ctx);
    this.seq = 1;

    this.port.postMessage({ type: 'host-hello', hostPubKey: Array.from(hostPubRaw) });
  }

  _onMsg(ev) {
    if (!this._pending) return;
    const { resolve, reject, aad } = this._pending;
    const d = ev.data || {};
    if (d.type === 'ciphertext-result') {
      open(this.session.aeadKey, deserializePayload(d.payload), aad)
        .then(res => { this._pending = null; resolve(res); })
        .catch(err => { this._pending = null; reject(err); });
    } else if (d.type === 'error') {
      this._pending = null;
      reject(new Error(d.error));
    }
  }

  async send(op, body, aad) {
    if (!this.session) throw new Error('no-session');
    const payload = await seal(this.session.aeadKey, this.session.baseIV, this.seq++, { op, ...body }, aad);
    const serialized = serializePayload(payload);
    this._lastSent = serialized;
    this.port.postMessage({ type: 'ciphertext-call', payload: serialized, seq: this.seq - 1 });
    return await new Promise((resolve, reject) => {
      this._pending = { resolve, reject, aad: `${aad}:result` };
      setTimeout(() => {
        if (this._pending) { this._pending = null; reject(new Error('timeout')); }
      }, 15000);
    });
  }

  lastPayload() { return this._lastSent; }
  resendRaw(serialized) { this.port.postMessage({ type: 'ciphertext-call', payload: serialized, seq: 999999 }); }
}
