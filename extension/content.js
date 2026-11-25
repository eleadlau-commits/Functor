(() => {
  const LANGS = { fn: 'auto', functor: 'auto', fnmath: 'math' };
  const ZOOM = 1.1;
  const PREAMBLE = /^\s*(set|sym|object|let|use|kind)\b/;
  const blocks = new Map();
  let enabled = true;
  let cssText = null;
  let timer = 0;
  let firstChange = 0;

  const CHAT_CSS = `
:host { display: block; color: inherit; padding: .2em 1em 1em; }
.bar { display: flex; gap: .6em; align-items: center; font: 12px/1.6 system-ui, sans-serif; opacity: .65; }
.bar:hover { opacity: 1; }
.bar .tag { font-weight: 600; letter-spacing: .02em; }
.bar .ms { flex: 1; }
.bar button { font: inherit; color: inherit; background: transparent; border: 1px solid currentColor; border-radius: 6px; padding: 0 .6em; cursor: pointer; opacity: .8; }
.out { zoom: ${ZOOM}; overflow-x: auto; overflow-y: hidden; }
.fn-doc { width: auto !important; padding: 0 !important; margin: 0 !important; background: transparent !important; color: inherit !important; --fn-ink: currentColor; --fn-muted: color-mix(in srgb, currentColor 65%, transparent); --fn-rule: color-mix(in srgb, currentColor 25%, transparent); }
.fn-doc .fn-code { background: color-mix(in srgb, currentColor 7%, transparent); }
.fn-style-quiet .fn-eqnum { right: 0; }
.fn-doc > :first-child { margin-top: 0; }
.fn-doc > :last-child { margin-bottom: 0; }
.diag { font: 12px/1.5 ui-monospace, monospace; color: #d0453c; margin-top: .4em; white-space: pre-wrap; }
`;

  const css = () => {
    if (!cssText) {
      cssText = fetch(chrome.runtime.getURL('doc.css')).then((r) => r.text()).then((doc) => doc + CHAT_CSS);
    }
    return cssText;
  };

  function addFonts() {
    if (document.getElementById('functor-fonts')) return;
    const url = (f) => chrome.runtime.getURL(`fonts/${f}.woff2`);
    const face = (family, style, weight, f) =>
      `@font-face{font-family:"${family}";font-style:${style};font-weight:${weight};font-display:swap;src:url("${url(f)}") format("woff2")}`;
    const s = document.createElement('style');
    s.id = 'functor-fonts';
    s.textContent = [
      face('Functor LM', 'normal', 400, 'functor-lm-regular'),
      face('Functor LM', 'italic', 400, 'functor-lm-italic'),
      face('Functor LM', 'normal', 700, 'functor-lm-bold'),
      face('Functor LM', 'italic', 700, 'functor-lm-bolditalic'),
      face('Functor LM Mono', 'normal', 400, 'functor-lm-mono'),
    ].join('\n');
    document.head.appendChild(s);
  }

  function langOf(pre) {
    const code = pre.querySelector('code') || pre;
    const cls = `${code.className} ${pre.className}`;
    const m = cls.match(/(?:^|\s)language-([\w-]+)/);
    if (m) return LANGS[m[1].toLowerCase()];
    const box = pre.parentElement;
    if (!box) return undefined;
    for (const el of box.querySelectorAll(':scope > *, :scope > * > *')) {
      if (el === pre || el.contains(pre)) continue;
      const t = el.textContent.trim().toLowerCase();
      if (t && t.length < 12 && LANGS[t]) return LANGS[t];
    }
    return undefined;
  }

  function prepare(src, mode) {
    let text = src.replace(/\s+$/, '');
    if (/^\s*(---|functor:)/.test(text)) return text;
    if (mode === 'math' || (mode === 'auto' && !text.includes('$') && !/^\s*#/m.test(text))) {
      const lines = text.split('\n');
      const head = [];
      while (lines.length && (PREAMBLE.test(lines[0]) || !lines[0].trim())) head.push(lines.shift());
      const body = lines.join('\n').trim();
      text = (head.length ? head.join('\n') + '\n\n' : '') + (body ? `$$\n${body}\n$$` : '');
    }
    return `---\nfunctor: 1\n---\n\n${text}\n`;
  }

  function clean(root) {
    root.querySelectorAll('script, iframe, object, embed, link, meta, style, foreignObject').forEach((n) => n.parentNode.removeChild(n));
    root.querySelectorAll('*').forEach((el) => {
      for (const a of [...el.attributes]) {
        const v = a.value.trim().toLowerCase();
        if (a.name.startsWith('on') || (/^(href|src|xlink:href|action)$/.test(a.name) && v.startsWith('javascript:'))) el.removeAttribute(a.name);
      }
    });
  }

  function compile(src, widthPt) {
    return chrome.runtime.sendMessage({ type: 'functor-compile', src, widthPt });
  }

  function show(pre, b) {
    const on = enabled && !b.source;
    pre.style.display = on ? 'none' : '';
    b.out.style.display = on ? '' : 'none';
    b.diag.style.display = on ? '' : 'none';
    b.btn.textContent = b.source ? 'Show math' : 'Show source';
    b.host.style.display = enabled ? '' : 'none';
  }

  async function renderBlock(pre, b) {
    const src = b.src;
    const width = Math.max(200, (b.host.clientWidth || pre.getBoundingClientRect().width || 600) / ZOOM);
    const out = await compile(prepare(src, b.mode), width * 0.75);
    if (b.src !== src || !out) return;
    b.width = width;
    const tpl = document.createElement('template');
    tpl.innerHTML = out.html || '';
    clean(tpl.content);
    b.out.replaceChildren(tpl.content);
    const errs = (out.diagnostics || []).filter((d) => d.severity === 'error');
    b.diag.textContent = errs.map((d) => `line ${d.line}: ${d.message}${d.hint ? ' ' + d.hint : ''}`).join('\n');
    b.ms.textContent = out.ms != null ? `${out.ms.toFixed(1)} ms` : '';
    show(pre, b);
  }

  async function makeBlock(pre, mode) {
    const host = document.createElement('div');
    host.className = 'functor-host';
    const root = host.attachShadow({ mode: 'open' });
    const style = document.createElement('style');
    style.textContent = await css();
    const bar = document.createElement('div');
    bar.className = 'bar';
    const tag = document.createElement('span');
    tag.className = 'tag';
    tag.textContent = 'Functor';
    const ms = document.createElement('span');
    ms.className = 'ms';
    const btn = document.createElement('button');
    btn.type = 'button';
    const out = document.createElement('div');
    out.className = 'out';
    const diag = document.createElement('div');
    diag.className = 'diag';
    bar.append(tag, ms, btn);
    root.append(style, bar, out, diag);
    const b = { host, out, diag, ms, btn, mode, src: null, source: false, width: 0 };
    btn.addEventListener('click', () => {
      b.source = !b.source;
      show(pre, b);
    });
    new ResizeObserver(() => {
      const w = host.clientWidth / ZOOM;
      if (b.src != null && !b.source && w > 0 && Math.abs(w - b.width) > 24) renderBlock(pre, b);
    }).observe(host);
    return b;
  }

  async function scanBlocks() {
    for (const pre of document.querySelectorAll('pre')) {
      if (pre.closest('.functor-host')) continue;
      let b = blocks.get(pre);
      if (!b) {
        const mode = langOf(pre);
        if (!mode) continue;
        b = await makeBlock(pre, mode);
        blocks.set(pre, b);
      }
      if (!b.host.isConnected || b.host.previousElementSibling !== pre) pre.after(b.host);
      const src = (pre.querySelector('code') || pre).textContent;
      if (src !== b.src) {
        b.src = src;
        renderBlock(pre, b);
      }
    }
    for (const [pre, b] of blocks) {
      if (!pre.isConnected) {
        b.host.replaceWith();
        blocks.delete(pre);
      }
    }
  }

  async function scanInline() {
    for (const code of document.querySelectorAll('code:not(pre code)')) {
      if (code.dataset.functor) continue;
      const m = code.textContent.match(/^fn:\s*([\s\S]+)$/);
      if (!m) continue;
      code.dataset.functor = '1';
      const out = await compile(prepare(`$${m[1].trim()}$`, 'doc'), 0);
      const tpl = document.createElement('template');
      tpl.innerHTML = (out && out.html) || '';
      const svg = tpl.content.querySelector('svg.fn-math');
      if (!svg) continue;
      clean(svg);
      const span = document.createElement('span');
      span.className = 'functor-inline';
      span.title = m[1].trim();
      span.style.cssText = 'cursor:pointer;font-size:1.1em;display:inline-block';
      span.append(svg);
      span.addEventListener('click', () => {
        span.style.display = 'none';
        code.style.display = '';
      });
      code.addEventListener('dblclick', () => {
        code.style.display = 'none';
        span.style.display = 'inline-block';
      });
      code.style.display = enabled ? 'none' : '';
      span.style.display = enabled ? 'inline-block' : 'none';
      code.after(span);
    }
  }

  function applyEnabled() {
    for (const [pre, b] of blocks) show(pre, b);
    document.querySelectorAll('code[data-functor]').forEach((code) => {
      const span = code.nextElementSibling;
      if (!span || !span.classList.contains('functor-inline')) return;
      code.style.display = enabled ? 'none' : '';
      span.style.display = enabled ? 'inline-block' : 'none';
    });
  }

  async function scan() {
    timer = 0;
    firstChange = 0;
    if (!enabled) return;
    addFonts();
    await scanBlocks();
    await scanInline();
  }

  function schedule() {
    const now = Date.now();
    if (!firstChange) firstChange = now;
    clearTimeout(timer);
    timer = setTimeout(scan, now - firstChange > 1500 ? 0 : 350);
  }

  new MutationObserver((list) => {
    if (list.every((m) => m.target.closest && m.target.closest('.functor-host, .functor-inline'))) return;
    schedule();
  }).observe(document.body, { childList: true, subtree: true, characterData: true });

  chrome.storage.local.get('enabled').then((v) => {
    enabled = v.enabled !== false;
    applyEnabled();
    scan();
  });
  chrome.storage.onChanged.addListener((c) => {
    if (!c.enabled) return;
    enabled = c.enabled.newValue !== false;
    applyEnabled();
    if (enabled) scan();
  });
})();
