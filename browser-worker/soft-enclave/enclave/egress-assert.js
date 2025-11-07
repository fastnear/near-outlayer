const ORIGIN = new URL(document.referrer || 'http://localhost:8080').origin;
const origPost = MessagePort.prototype.postMessage;
MessagePort.prototype.postMessage = function(msg, transfer) {
  if (msg?.type === 'ciphertext-result') {
    const p = msg.payload;
    const ok = p && Array.isArray(p.iv) && Array.isArray(p.ciphertext);
    if (!ok) throw new Error('ciphertext-result schema violation');
  } else if (msg?.type === 'error') {
    // allowed
  } else {
    throw new Error('plaintext egress blocked');
  }
  return origPost.call(this, msg, transfer);
};

const winPM = window.postMessage;
window.postMessage = function(msg, targetOrigin, transfer) {
  if (targetOrigin !== ORIGIN) throw new Error('unexpected target origin');
  return winPM.call(window, msg, targetOrigin, transfer);
};
