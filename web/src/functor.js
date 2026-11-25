const enc = new TextEncoder();
const dec = new TextDecoder();

export async function loadFunctor(url) {
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

  const glyphCache = new Map();
  const STORE = 'functor.glyphs';
  try {
    const saved = localStorage.getItem(STORE);
    if (saved) callRaw(w.cache_import, saved);
  } catch {}
  const persist = () => {
    const text = read(w.cache_export());
    if (!text) return;
    try {
      localStorage.setItem(STORE, text);
    } catch {}
  };

  return {
    compile(src, widthPt) {
      const t = performance.now();
      const out = JSON.parse(call(w.compile_width, src, widthPt || 0));
      out.ms = performance.now() - t;
      persist();
      return out;
    },
    pdf(src) {
      const len = callRaw(w.compile_pdf, src);
      return new Uint8Array(w.memory.buffer, w.result_ptr(), len).slice();
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
      initial: (last) => invoke('initial_file', { last }),
      load: (path) => invoke('load_file', { path }),
      open: () => invoke('open_file'),
      save: (path, text) => invoke('save_file', { path, text }),
      saveAs: (name, text) => invoke('save_as', { name, text }),
      exportHtml: (name, html) => invoke('export_html', { name, html }),
      exportPdf: (name, src) => invoke('export_pdf', { name, src }),
      close: (path) => invoke('close_file', { path }),
      askDiscard: (name) => invoke('ask_discard', { name }),
      onChange: (cb) => listen('file-changed', (e) => cb(e.payload)),
      onMenu: (cb) => listen('menu', (e) => cb(e.payload)),
    },
  };
}
