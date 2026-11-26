const enc = new TextEncoder();
const dec = new TextDecoder();

export async function instantiate(url) {
  const res = await fetch(url);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const w = instance.exports;
  const read = (len) => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), len));
  const callRaw = (fn, text, ...extra) => {
    const data = enc.encode(text);
    const ptr = w.alloc(data.length);
    new Uint8Array(w.memory.buffer, ptr, data.length).set(data);
    const len = fn(ptr, data.length, ...extra);
    w.dealloc(ptr, data.length);
    return len;
  };
  const call = (fn, text, ...extra) => read(callRaw(fn, text, ...extra));
  return { w, read, callRaw, call };
}
