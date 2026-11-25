const enc = new TextEncoder();
const dec = new TextDecoder();
let ready = null;

function load() {
  if (!ready) {
    ready = fetch(chrome.runtime.getURL('functor.wasm'))
      .then((r) => r.arrayBuffer())
      .then((b) => WebAssembly.instantiate(b, {}))
      .then(({ instance }) => instance.exports);
  }
  return ready;
}

function compile(w, src, widthPt) {
  const data = enc.encode(src);
  const ptr = w.alloc(data.length);
  new Uint8Array(w.memory.buffer, ptr, data.length).set(data);
  const len = w.compile_width(ptr, data.length, widthPt || 0);
  w.dealloc(ptr, data.length);
  return JSON.parse(dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), len)));
}

chrome.runtime.onMessage.addListener((msg, _sender, reply) => {
  if (msg.type !== 'functor-compile') return false;
  load()
    .then((w) => {
      const t = performance.now();
      const out = compile(w, msg.src, msg.widthPt);
      out.ms = performance.now() - t;
      reply(out);
    })
    .catch((e) => reply({ html: '', diagnostics: [{ severity: 'error', line: 0, col: 0, message: String(e) }] }));
  return true;
});

async function paint() {
  const { enabled = true } = await chrome.storage.local.get('enabled');
  chrome.action.setBadgeText({ text: enabled ? '' : 'off' });
  chrome.action.setBadgeBackgroundColor({ color: '#5b5b66' });
  chrome.action.setTitle({ title: enabled ? 'Functor: on (click to turn off)' : 'Functor: off (click to turn on)' });
}

chrome.action.onClicked.addListener(async () => {
  const { enabled = true } = await chrome.storage.local.get('enabled');
  await chrome.storage.local.set({ enabled: !enabled });
  paint();
});

chrome.runtime.onInstalled.addListener(paint);
chrome.runtime.onStartup.addListener(paint);
