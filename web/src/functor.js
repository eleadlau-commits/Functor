const enc = new TextEncoder();
const dec = new TextDecoder();

export async function loadFunctor(url) {
  const res = await fetch(url);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const w = instance.exports;

  const read = (len) => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), len));

  const call = (fn, text) => {
    const data = enc.encode(text);
    const ptr = w.alloc(data.length);
    new Uint8Array(w.memory.buffer, ptr, data.length).set(data);
    const len = fn(ptr, data.length);
    w.dealloc(ptr, data.length);
    return read(len);
  };

  const glyphCache = new Map();

  return {
    compile(src) {
      const t = performance.now();
      const out = JSON.parse(call(w.compile, src));
      out.ms = performance.now() - t;
      return out;
    },
    glyph(recipe) {
      if (!glyphCache.has(recipe)) glyphCache.set(recipe, call(w.glyph, recipe));
      return glyphCache.get(recipe);
    },
    symbols() {
      return JSON.parse(read(w.symbols()));
    },
  };
}
