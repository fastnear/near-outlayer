export const enc = (s) => new TextEncoder().encode(s);
export const dec = (b) => new TextDecoder().decode(b);
export const sha256 = async (b) => new Uint8Array(await crypto.subtle.digest("SHA-256", b));

export async function genECDH() {
  return crypto.subtle.generateKey({ name: "ECDH", namedCurve: "P-256" }, true, ["deriveBits"]);
}
export async function exportRaw(pub) {
  return new Uint8Array(await crypto.subtle.exportKey("raw", pub));
}
export async function importRaw(raw) {
  return crypto.subtle.importKey("raw", raw, { name: "ECDH", namedCurve: "P-256" }, true, []);
}

export async function deriveSession(priv, peerPub, ctx) {
  const ikm = new Uint8Array(await crypto.subtle.deriveBits({ name: "ECDH", public: peerPub }, priv, 256));
  const salt = await sha256(enc(`${ctx.hostOrigin}|${ctx.enclaveOrigin}|${ctx.codeHash}`));
  const hkdfKey = await crypto.subtle.importKey("raw", ikm, "HKDF", false, ["deriveKey","deriveBits"]);
  const aeadKey = await crypto.subtle.deriveKey(
    { name: "HKDF", hash: "SHA-256", salt, info: enc("soft-enclave/aead") },
    hkdfKey, { name: "AES-GCM", length: 256 }, false, ["encrypt","decrypt"]
  );
  const baseIV = new Uint8Array(await crypto.subtle.deriveBits(
    { name: "HKDF", hash: "SHA-256", salt, info: enc("soft-enclave/iv") },
    hkdfKey, 96
  ));
  ikm.fill(0);
  return { aeadKey, baseIV };
}

function ivFrom(baseIV, seq) {
  const iv = new Uint8Array(baseIV);
  let x = seq >>> 0;
  for (let i=0;i<4;i++){ iv[11-i] ^= (x & 0xff); x >>>= 8; }
  return iv;
}

export async function seal(aeadKey, baseIV, seq, body, aad="") {
  const iv = ivFrom(baseIV, seq);
  const ct = await crypto.subtle.encrypt({ name:"AES-GCM", iv, additionalData: enc(aad) },
                                         aeadKey, enc(JSON.stringify(body)));
  return { iv, ciphertext: new Uint8Array(ct) };
}

export async function open(aeadKey, payload, aad="") {
  const pt = await crypto.subtle.decrypt({ name:"AES-GCM", iv: payload.iv, additionalData: enc(aad) },
                                         aeadKey, payload.ciphertext);
  return JSON.parse(dec(new Uint8Array(pt)));
}

export const serializePayload = (p) => ({ iv: Array.from(p.iv), ciphertext: Array.from(p.ciphertext) });
export const deserializePayload = (s) => ({ iv: new Uint8Array(s.iv), ciphertext: new Uint8Array(s.ciphertext) });
