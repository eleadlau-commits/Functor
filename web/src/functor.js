import { instantiate } from './wasm.js';
import { createParts, compileParts } from './parts.js';

const STORE = 'functor.glyphs';

function savedGlyphs() {
  try {
    return localStorage.getItem(STORE) || '';
  } catch {
    return '';
  }
}

function keepGlyphs(text) {
  if (!text) return;
  try {
    localStorage.setItem(STORE, text);
  } catch {}
}

function startWorker(url) {
  let worker;
  try {
    worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
  } catch {
    return null;
  }
  const waiting = new Map();
  let next = 1;
  worker.onmessage = (e) => {
    const { id, out, cache } = e.data;
    keepGlyphs(cache);
    const done = waiting.get(id);
    waiting.delete(id);
    if (done) done(out);
  };
  worker.postMessage({ type: 'init', url, cache: savedGlyphs() });
  return (src, width, reset) =>
    new Promise((resolve) => {
      const id = next++;
      waiting.set(id, resolve);
      worker.postMessage({ type: 'compile', id, src, width, reset });
    });
}

export async function loadFunctor(url) {
  const { w, read, callRaw, call } = await instantiate(url);
  const saved = savedGlyphs();
  if (saved) callRaw(w.cache_import, saved);
  const persist = () => keepGlyphs(read(w.cache_export()));
  const glyphCache = new Map();
  const parts = createParts();
  const here = (src, width, reset) => {
    if (reset) w.session_reset();
    const t = performance.now();
    const out = JSON.parse(call(w.session_compile, src, width || 0));
    out.ms = performance.now() - t;
    persist();
    return out;
  };
  const run = startWorker(url) || here;

  return {
    worker: run !== here,
    compile(src, widthPt) {
      return compileParts(parts, run, src, widthPt);
    },
    compileFull(src, widthPt) {
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
  const parts = createParts();
  const run = async (src, width, reset) => {
    const t = performance.now();
    const out = JSON.parse(await invoke('compile_parts', { src, width: width || null, reset }));
    out.ms = performance.now() - t;
    return out;
  };
  return {
    native: true,
    setTitle: (title) => getCurrentWindow().setTitle(title),
    compile(src, widthPt) {
      return compileParts(parts, run, src, widthPt);
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
