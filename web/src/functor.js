const enc = new TextEncoder();
const dec = new TextDecoder();

export async function loadFunctor(url) {
  const res = await fetch(url);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const w = instance.exports;

  const read = (len) => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), len));

  const call = (fn, text, ...extra) => {
    const data = enc.encode(text);
    const ptr = w.alloc(data.length);
    new Uint8Array(w.memory.buffer, ptr, data.length).set(data);
    const len = fn(ptr, data.length, ...extra);
    w.dealloc(ptr, data.length);
    return read(len);
  };

  const glyphCache = new Map();

  return {
    compile(src, widthPt) {
      const t = performance.now();
      const out = JSON.parse(call(w.compile_width, src, widthPt || 0));
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

export const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function loadNative() {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const glyphCache = new Map();
  return {
    native: true,
    setTitle: (title) => getCurrentWindow().setTitle(title),
    async compile(src, widthPt) {
      const t = performance.now();
      const out = JSON.parse(await invoke('compile', { src, width: widthPt || null }));
      out.ms = performance.now() - t;
      return out;
    },
    async glyph(recipe) {
      if (!glyphCache.has(recipe)) glyphCache.set(recipe, await invoke('glyph', { recipe }));
      return glyphCache.get(recipe);
    },
    async symbols() {
      return JSON.parse(await invoke('symbols'));
    },
    files: {
      initial: () => invoke('initial_file'),
      open: () => invoke('open_file'),
      save: (path, text) => invoke('save_file', { path, text }),
      saveAs: (name, text) => invoke('save_as', { name, text }),
      exportHtml: (name, html) => invoke('export_html', { name, html }),
      close: () => invoke('close_file'),
      onChange: (cb) => listen('file-changed', (e) => cb(e.payload)),
    },
  };
}
