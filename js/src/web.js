const fnScript = document.currentScript;
const fnBase = (globalThis.FunctorConfig && globalThis.FunctorConfig.base) || (fnScript && fnScript.src ? new URL('.', fnScript.src).href : './');
const fnEnc = new TextEncoder();
const fnDec = new TextDecoder();
let fnWasm = null;

function fnLoadScript(url) {
  return new Promise((ok, bad) => {
    const s = document.createElement('script');
    s.src = url;
    s.onload = ok;
    s.onerror = () => bad(new Error(`cannot load ${url}`));
    document.head.appendChild(s);
  });
}

function fnBytes(b64) {
  const s = atob(b64);
  const u = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i);
  return u;
}

async function fnWasmBytes() {
  if (globalThis.__functorWasm) return fnBytes(globalThis.__functorWasm);
  try {
    const r = await fetch(fnBase + 'functor.wasm');
    if (!r.ok) throw new Error(r.statusText);
    return new Uint8Array(await r.arrayBuffer());
  } catch {
    await fnLoadScript(fnBase + 'functor-wasm.js');
    return fnBytes(globalThis.__functorWasm);
  }
}

function fnEngine() {
  if (!fnWasm) {
    fnWasm = fnWasmBytes()
      .then((b) => WebAssembly.instantiate(b, {}))
      .then(({ instance }) => {
        const w = instance.exports;
        const read = (len) => fnDec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), len));
        const callRaw = (f, text, ...extra) => {
          const data = fnEnc.encode(text);
          const ptr = w.alloc(data.length);
          new Uint8Array(w.memory.buffer, ptr, data.length).set(data);
          const len = f(ptr, data.length, ...extra);
          w.dealloc(ptr, data.length);
          return len;
        };
        const STORE = 'functor.glyphs';
        try {
          const saved = localStorage.getItem(STORE);
          if (saved) callRaw(w.cache_import, saved);
        } catch {}
        return {
          compile(src, widthPt) {
            const t = performance.now();
            const out = JSON.parse(read(callRaw(w.compile_width, src, widthPt || 0)));
            out.ms = performance.now() - t;
            try {
              const text = read(w.cache_export());
              if (text) localStorage.setItem(STORE, text);
            } catch {}
            return out;
          },
        };
      });
  }
  return fnWasm;
}

const fnR = fnRenderer({
  compile: (src, widthPt) => fnEngine().then((e) => e.compile(src, widthPt)),
  css: __FUNCTOR_CSS__,
  fontUrl: (f) => `data:font/woff2;base64,${__FUNCTOR_FONTS__[f]}`,
  onRender: (el, out) => el.dispatchEvent(new CustomEvent('functor:render', { bubbles: true, detail: { diagnostics: out.diagnostics || [] } })),
});

class FnMathElement extends HTMLElement {
  connectedCallback() {
    fnR.addFonts();
    this.update();
    if (!this.__fnObs) {
      this.__fnObs = new MutationObserver(() => this.update());
      this.__fnObs.observe(this, { childList: true, subtree: true, characterData: true });
    }
  }
  static get observedAttributes() {
    return ['display'];
  }
  attributeChangedCallback() {
    if (this.isConnected) this.update(true);
  }
  update(reset) {
    if (reset && this.shadowRoot) {
      this.shadowRoot.replaceChildren();
      this.__fnSrc = null;
    }
    if (this.hasAttribute('display')) fnR.element(this, 'math');
    else fnR.inline(this, this.textContent);
  }
}

class FnDocElement extends HTMLElement {
  connectedCallback() {
    fnR.addFonts();
    fnR.element(this, 'doc');
    if (!this.__fnObs) {
      this.__fnObs = new MutationObserver(() => fnR.element(this, 'doc'));
      this.__fnObs.observe(this, { childList: true, subtree: true, characterData: true });
    }
  }
}

if (globalThis.customElements && !customElements.get('fn-math')) {
  customElements.define('fn-math', FnMathElement);
  customElements.define('fn-doc', FnDocElement);
}

const fnHead = document.createElement('style');
fnHead.textContent = 'fn-math:not(:defined),fn-doc:not(:defined){visibility:hidden}fn-math[display],fn-doc{display:block}';
document.head.appendChild(fnHead);

const fnAuto = !(fnScript && fnScript.dataset.auto === 'false') && !(globalThis.FunctorConfig && globalThis.FunctorConfig.auto === false);

globalThis.Functor = {
  version: __FUNCTOR_VERSION__,
  ready: () => fnEngine().then(() => true),
  async compile(src, widthPt) {
    return (await fnEngine()).compile(src, widthPt);
  },
  async render(src, options) {
    const opt = options || {};
    if (!opt.display && !opt.doc) {
      const inl = await fnR.compile(fnPrepare(`$${src.trim()}$`, 'doc'), 0);
      const svg = fnErrors(inl).length ? null : fnInlineMath(inl);
      if (!svg) throw new Error(fnDiagText(inl) || 'no math');
      return svg.outerHTML;
    }
    const out = await fnR.compile(fnPrepare(src, opt.display ? 'math' : 'doc'), opt.widthPt || 0);
    if (fnErrors(out).length) throw new Error(fnDiagText(out));
    const tpl = document.createElement('template');
    tpl.innerHTML = out.html || '';
    fnClean(tpl.content);
    return tpl.innerHTML;
  },
  renderAll: (root) => fnR.scan(root),
  fontFaces: () => fnFontFaces((f) => `data:font/woff2;base64,${__FUNCTOR_FONTS__[f]}`),
};

function fnStart() {
  fnR.addFonts();
  if (!fnAuto) return;
  fnR.scan();
  fnR.observe(document.body);
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', fnStart);
else fnStart();
