/*
 * engine-worker.js — runs the RSAShield Rust core (rsashield.wasm) off the main
 * thread. Messages carry the same method/path/body as an HTTP call to the local
 * server; replies carry { id, status, body }. Nothing here touches the network
 * except fetching the module itself, so audited keys never leave the device.
 */
'use strict';

const enc = new TextEncoder();
const dec = new TextDecoder();
let engine = null;

async function load() {
  const imports = { env: { rsashield_now_ms: () => performance.now() } };
  try {
    const { instance } = await WebAssembly.instantiateStreaming(fetch('rsashield.wasm'), imports);
    return instance.exports;
  } catch {
    // servers that send a non-wasm MIME type: fall back to compiling from bytes
    const res = await fetch('rsashield.wasm');
    if (!res.ok) throw new Error(`could not load rsashield.wasm (HTTP ${res.status})`);
    const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), imports);
    return instance.exports;
  }
}

// copy a string into wasm memory; returns [ptr, len]
function put(x, str) {
  const bytes = enc.encode(str);
  if (!bytes.length) return [0, 0];
  const ptr = x.rsashield_alloc(bytes.length);
  new Uint8Array(x.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

self.onmessage = async (event) => {
  const { id, method, path, body } = event.data;
  try {
    engine = engine || load();
    const x = await engine;
    const args = [method, path, body || ''].map((s) => put(x, s));
    const status = x.rsashield_call(...args.flat());
    for (const [ptr, len] of args) if (len) x.rsashield_free(ptr, len);
    // read after the call: memory may have grown and replaced the old buffer
    const out = new Uint8Array(x.memory.buffer, x.rsashield_out_ptr(), x.rsashield_out_len()).slice();
    self.postMessage({ id, status, body: dec.decode(out) });
  } catch (err) {
    engine = null; // a trap leaves the instance unusable; start fresh next time
    self.postMessage({ id, status: 500, body: JSON.stringify({ error: `engine error: ${err.message}` }) });
  }
};
