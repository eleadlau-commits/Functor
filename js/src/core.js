const FN_LANGS = { fn: 'auto', functor: 'auto', fnmath: 'math' };
const FN_PREAMBLE = /^\s*(set|sym|object|let|use|kind)\b/;
const FN_TEXT_PX = (11 * 96) / 72;
const FN_HOSTS = '.functor-host, .functor-inline, fn-math, fn-doc';

const FN_EMBED_CSS = `
:host { display: block; color: inherit; }
:host(.functor-host) { padding: .2em 1em 1em; }
:host([hidden]) { display: none; }
.bar { display: flex; gap: .6em; align-items: center; font: 12px/1.6 system-ui, sans-serif; opacity: .65; }
.bar:hover { opacity: 1; }
.bar .tag { font-weight: 600; letter-spacing: .02em; }
.bar .ms { flex: 1; }
.bar button { font: inherit; color: inherit; background: transparent; border: 1px solid currentColor; border-radius: 6px; padding: 0 .6em; cursor: pointer; opacity: .8; }
.out { overflow-x: auto; overflow-y: hidden; }
.fn-doc { width: auto !important; padding: 0 !important; margin: 0 !important; background: transparent !important; color: inherit !important; --fn-ink: currentColor; --fn-muted: color-mix(in srgb, currentColor 65%, transparent); --fn-rule: color-mix(in srgb, currentColor 25%, transparent); --fn-code-bg: color-mix(in srgb, currentColor 7%, transparent); }
.fn-doc .fn-code { background: var(--fn-code-bg); }
.fn-style-quiet .fn-eqnum { right: 0; }
.fn-doc > :first-child { margin-top: 0; }
.fn-doc > :last-child { margin-bottom: 0; }
.diag { font: 12px/1.5 ui-monospace, monospace; color: #d0453c; margin-top: .4em; white-space: pre-wrap; }
.diag:empty { display: none; }
.src { font: .9em ui-monospace, monospace; color: #d0453c; }
`;

const FN_INLINE_CSS = `
:host { display: inline; }
.fn-math { display: inline-block; overflow: visible; }
.src { font: .9em ui-monospace, monospace; color: #d0453c; }
`;

const FN_FONTS = [
  ['Functor LM', 'normal', 400, 'functor-lm-regular'],
  ['Functor LM', 'italic', 400, 'functor-lm-italic'],
  ['Functor LM', 'normal', 700, 'functor-lm-bold'],
  ['Functor LM', 'italic', 700, 'functor-lm-bolditalic'],
  ['Functor LM Mono', 'normal', 400, 'functor-lm-mono'],
];

function fnFontFaces(url) {
  return FN_FONTS.map(
    ([family, style, weight, f]) =>
      `@font-face{font-family:"${family}";font-style:${style};font-weight:${weight};font-display:swap;src:url("${url(f)}") format("woff2")}`,
  ).join('\n');
}

function fnPrepare(src, mode) {
  let text = src.replace(/\s+$/, '');
  if (/^\s*(---|functor:)/.test(text)) return text;
  if (mode === 'math' || (mode === 'auto' && !text.includes('$') && !/^\s*#/m.test(text))) {
    const lines = text.split('\n');
    const head = [];
    while (lines.length && (FN_PREAMBLE.test(lines[0]) || !lines[0].trim())) head.push(lines.shift());
    const body = lines.join('\n').trim();
    text = (head.length ? head.join('\n') + '\n\n' : '') + (body ? `$$\n${body}\n$$` : '');
  }
  return `---\nfunctor: 1\n---\n\n${text}\n`;
}

function fnClean(root) {
  root.querySelectorAll('script, iframe, object, embed, link, meta, style, foreignObject').forEach((n) => n.remove());
  const all = root.querySelectorAll ? [root, ...root.querySelectorAll('*')] : [];
  for (const el of all) {
    if (!el.attributes) continue;
    for (const a of [...el.attributes]) {
      const v = a.value.trim().toLowerCase();
      if (a.name.startsWith('on') || (/^(href|src|xlink:href|action)$/.test(a.name) && v.startsWith('javascript:'))) el.removeAttribute(a.name);
    }
  }
}

function fnErrors(out) {
  return ((out && out.diagnostics) || []).filter((d) => d.severity === 'error');
}

function fnDiagText(out) {
  return fnErrors(out)
    .map((d) => `line ${d.line}: ${d.message}${d.hint ? ' ' + d.hint : ''}`)
    .join('\n');
}

function fnInlineMath(out) {
  const tpl = document.createElement('template');
  tpl.innerHTML = (out && out.html) || '';
  const line = tpl.content.querySelector('.fn-line') || tpl.content.querySelector('.fn-par');
  if (!line || !line.querySelector('svg.fn-math')) return null;
  const span = document.createElement('span');
  span.className = 'fn-inline';
  span.style.whiteSpace = 'nowrap';
  span.append(...line.childNodes);
  fnClean(span);
  return span;
}

function fnRenderer(opt) {
  const o = { toolbar: false, labels: false, zoom: 'auto', ...opt };
  const blocks = new Map();
  const cache = new Map();
  let enabled = true;
  let cssText = null;
  let timer = 0;
  let firstChange = 0;
  let fontsAdded = false;

  const css = () => {
    if (!cssText) cssText = Promise.resolve(o.css).then((doc) => (doc || '') + FN_EMBED_CSS);
    return cssText;
  };

  function addFonts() {
    if (fontsAdded || !o.fontUrl || document.getElementById('functor-fonts')) return;
    fontsAdded = true;
    const s = document.createElement('style');
    s.id = 'functor-fonts';
    s.textContent = fnFontFaces(o.fontUrl);
    document.head.appendChild(s);
  }

  function compile(src, widthPt) {
    const key = `${Math.round(widthPt || 0)}\n${src}`;
    if (!cache.has(key)) {
      if (cache.size > 400) cache.clear();
      const p = Promise.resolve()
        .then(() => o.compile(src, widthPt))
        .catch((e) => ({ html: '', diagnostics: [{ severity: 'error', line: 0, col: 0, message: String(e) }] }));
      cache.set(key, p);
    }
    return cache.get(key);
  }

  function zoomOf(el) {
    if (typeof o.zoom === 'number') return o.zoom;
    const px = parseFloat(getComputedStyle(el).fontSize) || FN_TEXT_PX;
    return px / FN_TEXT_PX;
  }

  function langOf(pre) {
    const code = pre.querySelector('code') || pre;
    const cls = `${code.className} ${pre.className}`;
    const m = cls.match(/(?:^|\s)(?:language|lang)-([\w-]+)/);
    if (m) return FN_LANGS[m[1].toLowerCase()];
    if (!o.labels) return undefined;
    const box = pre.parentElement;
    if (!box) return undefined;
    for (const el of box.querySelectorAll(':scope > *, :scope > * > *')) {
      if (el === pre || el.contains(pre)) continue;
      const t = el.textContent.trim().toLowerCase();
      if (t && t.length < 12 && FN_LANGS[t]) return FN_LANGS[t];
    }
    return undefined;
  }

  async function makeBox(host, mode, pre) {
    const root = host.shadowRoot || host.attachShadow({ mode: 'open' });
    const style = document.createElement('style');
    style.textContent = await css();
    const out = document.createElement('div');
    out.className = 'out';
    const diag = document.createElement('div');
    diag.className = 'diag';
    const b = { host, pre, out, diag, mode, src: null, source: false, width: 0, ms: null, btn: null };
    root.replaceChildren(style);
    if (o.toolbar && pre) {
      const bar = document.createElement('div');
      bar.className = 'bar';
      const tag = document.createElement('span');
      tag.className = 'tag';
      tag.textContent = 'Functor';
      b.ms = document.createElement('span');
      b.ms.className = 'ms';
      b.btn = document.createElement('button');
      b.btn.type = 'button';
      b.btn.addEventListener('click', () => {
        b.source = !b.source;
        show(b);
      });
      bar.append(tag, b.ms, b.btn);
      root.append(bar);
    }
    root.append(out, diag);
    new ResizeObserver(() => {
      const w = host.clientWidth / zoomOf(host);
      if (b.src != null && !b.source && w > 0 && Math.abs(w - b.width) > 24) renderBox(b);
    }).observe(host);
    return b;
  }

  function show(b) {
    const on = enabled && !b.source;
    if (b.pre) b.pre.style.display = on ? 'none' : '';
    b.out.style.display = on ? '' : 'none';
    b.diag.style.display = on ? '' : 'none';
    if (b.btn) b.btn.textContent = b.source ? 'Show math' : 'Show source';
    if (b.pre) b.host.style.display = enabled ? '' : 'none';
  }

  async function renderBox(b) {
    const src = b.src;
    const z = zoomOf(b.host);
    const width = Math.max(160, (b.host.clientWidth || (b.pre && b.pre.getBoundingClientRect().width) || 600) / z);
    const out = await compile(fnPrepare(src, b.mode), width * 0.75);
    if (b.src !== src) return;
    b.width = width;
    b.out.style.zoom = z;
    const tpl = document.createElement('template');
    tpl.innerHTML = out.html || '';
    fnClean(tpl.content);
    b.out.replaceChildren(tpl.content);
    b.diag.textContent = fnDiagText(out);
    if (b.ms) b.ms.textContent = out.ms != null ? `${out.ms.toFixed(1)} ms` : '';
    show(b);
    if (o.onRender) o.onRender(b.host, out);
  }

  async function element(el, mode) {
    let b = blocks.get(el);
    if (!b) {
      b = await makeBox(el, mode, null);
      blocks.set(el, b);
    }
    const src = el.textContent;
    if (src !== b.src) {
      b.src = src;
      await renderBox(b);
    }
    return b;
  }

  async function inline(host, src) {
    const root = host.shadowRoot || host.attachShadow({ mode: 'open' });
    const text = src.trim();
    if (host.__fnSrc === text) return;
    host.__fnSrc = text;
    const out = await compile(fnPrepare(`$${text}$`, 'doc'), 0);
    if (host.__fnSrc !== text) return;
    const style = document.createElement('style');
    style.textContent = FN_INLINE_CSS;
    const svg = fnErrors(out).length ? null : fnInlineMath(out);
    if (svg) {
      root.replaceChildren(style, svg);
    } else {
      const s = document.createElement('span');
      s.className = 'src';
      s.textContent = text;
      s.title = fnDiagText(out);
      root.replaceChildren(style, s);
    }
    if (o.onRender) o.onRender(host, out);
  }

  async function scanBlocks(scope) {
    for (const pre of scope.querySelectorAll('pre')) {
      if (pre.closest(FN_HOSTS)) continue;
      let b = blocks.get(pre);
      if (!b) {
        const mode = langOf(pre);
        if (!mode) continue;
        const host = document.createElement('div');
        host.className = 'functor-host';
        b = await makeBox(host, mode, pre);
        blocks.set(pre, b);
      }
      if (!b.host.isConnected || b.host.previousElementSibling !== pre) pre.after(b.host);
      const src = (pre.querySelector('code') || pre).textContent;
      if (src !== b.src) {
        b.src = src;
        renderBox(b);
      }
    }
    for (const [key, b] of blocks) {
      if (!key.isConnected) {
        if (b.pre) b.host.remove();
        blocks.delete(key);
      }
    }
  }

  async function scanInline(scope) {
    for (const code of scope.querySelectorAll('code:not(pre code)')) {
      if (code.dataset.functor || code.closest(FN_HOSTS)) continue;
      const m = code.textContent.match(/^fn:\s*([\s\S]+)$/);
      if (!m) continue;
      code.dataset.functor = '1';
      const span = document.createElement('span');
      span.className = 'functor-inline';
      span.title = m[1].trim();
      span.style.cssText = `cursor:pointer;font-size:${typeof o.zoom === 'number' ? o.zoom : 1}em`;
      span.addEventListener('click', () => {
        span.style.display = 'none';
        code.style.display = '';
      });
      code.addEventListener('dblclick', () => {
        code.style.display = 'none';
        span.style.display = '';
      });
      code.after(span);
      code.style.display = enabled ? 'none' : '';
      span.style.display = enabled ? '' : 'none';
      inline(span, m[1]);
    }
  }

  function setEnabled(on) {
    enabled = on;
    for (const b of blocks.values()) show(b);
    document.querySelectorAll('code[data-functor]').forEach((code) => {
      const span = code.nextElementSibling;
      if (!span || !span.classList.contains('functor-inline')) return;
      code.style.display = enabled ? 'none' : '';
      span.style.display = enabled ? '' : 'none';
    });
    if (on) scan();
  }

  async function scan(scope) {
    timer = 0;
    firstChange = 0;
    if (!enabled) return;
    addFonts();
    const s = scope || document.body;
    await scanBlocks(s);
    await scanInline(s);
  }

  function schedule() {
    const now = Date.now();
    if (!firstChange) firstChange = now;
    clearTimeout(timer);
    timer = setTimeout(() => scan(), now - firstChange > 1500 ? 0 : o.delay ?? 350);
  }

  function observe(target) {
    new MutationObserver((list) => {
      if (list.every((m) => m.target.closest && m.target.closest(FN_HOSTS))) return;
      schedule();
    }).observe(target || document.body, { childList: true, subtree: true, characterData: true });
  }

  return { scan, observe, setEnabled, element, inline, compile, addFonts, css };
}
