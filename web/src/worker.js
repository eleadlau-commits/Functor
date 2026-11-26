import { instantiate } from './wasm.js';

let ready = null;

self.onmessage = async (e) => {
  const m = e.data;
  if (m.type === 'init') {
    ready = instantiate(m.url).then((x) => {
      if (m.cache) x.callRaw(x.w.cache_import, m.cache);
      return x;
    });
    return;
  }
  const x = await ready;
  if (m.type === 'compile') {
    if (m.reset) x.w.session_reset();
    const t = performance.now();
    const out = JSON.parse(x.call(x.w.session_compile, m.src, m.width || 0));
    out.ms = performance.now() - t;
    const cache = x.read(x.w.cache_export());
    self.postMessage({ id: m.id, out, cache });
  }
};
