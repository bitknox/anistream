// What the TV's QR pages seal a phone's answer with, and open what the TV fills in with:
// AES-256-GCM under the one-time key in the page address's #fragment, which a browser never
// sends. The pages are plain HTTP on the local network, where browsers do not offer
// crypto.subtle, so the cipher is @noble/ciphers' (audited by Cure53), bundled by build.sh into
// the one script the TV serves inline. The TV opens it with the platform's own AES/GCM.
import { gcm } from '@noble/ciphers/aes.js';

const NONCE = 12;
const encoder = new TextEncoder();

function fromBase64Url(text) {
  const plain = text.replace(/-/g, '+').replace(/_/g, '/');
  const binary = atob(plain + '='.repeat((4 - (plain.length % 4)) % 4));
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

function toBase64Url(bytes) {
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** The key in a page address's fragment, "#k=" and 43 characters: its 32 bytes, or null. */
function keyFrom(hash) {
  const found = /^#k=([A-Za-z0-9_-]{43})$/.exec(hash || '');
  if (!found) return null;
  const key = fromBase64Url(found[1]);
  return key.length === 32 ? key : null;
}

/** [text] sealed for the page at [path]: a fresh 12-byte nonce, then the ciphertext and its tag, in base64url. */
function seal(key, path, text) {
  const nonce = crypto.getRandomValues(new Uint8Array(NONCE));
  const sealed = gcm(key, nonce, encoder.encode(path)).encrypt(encoder.encode(text));
  const out = new Uint8Array(NONCE + sealed.length);
  out.set(nonce);
  out.set(sealed, NONCE);
  return toBase64Url(out);
}

/** What [seal] made, opened; it throws when the key, the path or the bytes are not the ones sealed. */
function open(key, path, sealed) {
  const bytes = fromBase64Url(sealed);
  if (bytes.length < NONCE + 16) throw new Error('too short to be sealed');
  const text = gcm(key, bytes.subarray(0, NONCE), encoder.encode(path)).decrypt(bytes.subarray(NONCE));
  return new TextDecoder().decode(text);
}

globalThis.anistreamPage = { keyFrom, seal, open };
