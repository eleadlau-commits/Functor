const LAYERS = [
  ['Meaning', (t) => [t.meaning]],
  ['Style', (t) => (t.style.length ? t.style : ['No style rule. The look comes from the recipe.'])],
  ['Construction', (t) => [t.recipe, ...t.ops.map((o) => `.${o.name}: ${o.meaning}`)]],
  ['Glyph', (t) => [t.glyph]],
];

function code(text) {
  const frag = document.createDocumentFragment();
  text.split(/(`[^`]*`)/).forEach((part) => {
    if (part.startsWith('`') && part.endsWith('`') && part.length > 1) {
      const c = document.createElement('code');
      c.textContent = part.slice(1, -1);
      frag.append(c);
    } else if (part) {
      frag.append(part);
    }
  });
  return frag;
}

export function traceAt(traces, line, col) {
  let best = null;
  for (const t of traces) {
    const after = line > t.line || (line === t.line && col >= t.col);
    const before = line < t.endLine || (line === t.endLine && col <= t.endCol);
    if (!after || !before) continue;
    const size = (t.endLine - t.line) * 1000 + t.endCol - t.col;
    if (!best || size < best.size) best = { t, size };
  }
  return best && best.t;
}

export function traceKey(t) {
  return t ? `${t.line}:${t.col}:${t.source}:${t.recipe}:${t.style.join('|')}` : null;
}

export function renderTrace(root, t, svg) {
  if (!t) {
    const empty = document.createElement('p');
    empty.className = 'dock-empty';
    empty.textContent = 'Put the cursor on a symbol to see its four layers: meaning, style, construction and glyph.';
    root.replaceChildren(empty);
    return;
  }
  const head = document.createElement('div');
  head.className = 'insp-head';
  const pic = document.createElement('div');
  pic.className = 'insp-glyph';
  if (svg) pic.innerHTML = svg;
  const src = document.createElement('code');
  src.className = 'insp-source';
  src.textContent = t.source;
  const form = document.createElement('span');
  form.className = `insp-form insp-${t.form}`;
  form.textContent = t.form === 'hand' ? 'no meaning in the source' : t.form;
  head.append(pic, src, form);
  const rows = document.createElement('ol');
  rows.className = 'insp-layers';
  for (const [name, lines] of LAYERS) {
    const li = document.createElement('li');
    const h = document.createElement('span');
    h.className = 'insp-name';
    h.textContent = name;
    li.append(h);
    lines(t).forEach((text, i) => {
      const p = document.createElement('span');
      p.className = i === 0 ? 'insp-main' : 'insp-op';
      p.append(code(text));
      li.append(p);
    });
    rows.append(li);
  }
  root.replaceChildren(head, rows);
}

export function createInspector(fn, sink) {
  let shown;
  let seq = 0;
  return async function show(t) {
    const key = traceKey(t);
    if (key === shown) return;
    shown = key;
    const mine = ++seq;
    sink(t || null, '');
    const recipe = t && /^[^\s(]+$/.test(t.recipe) ? t.recipe : null;
    if (!recipe) return;
    const svg = await fn.glyph(recipe);
    if (mine === seq && svg) sink(t, svg);
  };
}
