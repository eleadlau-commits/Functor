export function createParts() {
  let cache = new Map();
  return {
    take(out) {
      const next = new Map();
      let whole = true;
      for (const p of out.parts) {
        const piece = p.html !== undefined ? { html: p.html, traces: p.traces || [] } : next.get(p.key) || cache.get(p.key);
        if (!piece) {
          whole = false;
          continue;
        }
        next.set(p.key, piece);
        p.html = piece.html;
        p.rel = piece.traces;
        delete p.traces;
      }
      cache = next;
      return whole;
    },
  };
}

export function finish(out) {
  out.traces = [];
  for (const p of out.parts) {
    const off = p.line - 1;
    for (const t of p.rel) out.traces.push(off ? { ...t, line: t.line + off, endLine: t.endLine + off } : t);
  }
  Object.defineProperty(out, 'html', {
    configurable: true,
    enumerable: true,
    get() {
      return out.open + out.parts.map((p) => p.html).join('') + '</article>';
    },
  });
  return out;
}

export async function compileParts(parts, run, src, width) {
  let out = await run(src, width, false);
  if (!parts.take(out)) {
    out = await run(src, width, true);
    parts.take(out);
  }
  return finish(out);
}

function nodesOf(html) {
  const t = document.createElement('template');
  t.innerHTML = html;
  return [...t.content.childNodes];
}

function sameAttrs(el, open) {
  const t = document.createElement('template');
  t.innerHTML = open + '</article>';
  const fresh = t.content.firstElementChild;
  for (const a of [...el.attributes]) if (!fresh.hasAttribute(a.name)) el.removeAttribute(a.name);
  for (const a of fresh.attributes) if (el.getAttribute(a.name) !== a.value) el.setAttribute(a.name, a.value);
}

function mark(entry) {
  const line = String(entry.line);
  const end = String(entry.endLine);
  for (const n of entry.nodes) {
    if (n.nodeType !== 1) continue;
    if (n.dataset.fnLine !== line) n.dataset.fnLine = line;
    if (n.dataset.fnEndLine !== end) n.dataset.fnEndLine = end;
  }
}

export function patch(root, out) {
  let state = root.__fnParts;
  let article = state && state.article;
  if (!article || article.parentNode !== root) {
    root.innerHTML = out.open + '</article>';
    article = root.firstElementChild;
    state = { article, open: out.open, entries: [] };
    root.__fnParts = state;
  } else if (state.open !== out.open) {
    sameAttrs(article, out.open);
    state.open = out.open;
  }
  const old = state.entries;
  const parts = out.parts;
  let head = 0;
  while (head < old.length && head < parts.length && old[head].key === parts[head].key) head++;
  let tail = 0;
  while (tail < old.length - head && tail < parts.length - head && old[old.length - 1 - tail].key === parts[parts.length - 1 - tail].key) tail++;
  const spare = new Map();
  for (const e of old.slice(head, old.length - tail)) {
    if (!spare.has(e.key)) spare.set(e.key, []);
    spare.get(e.key).push(e);
  }
  let anchor = null;
  for (const e of old.slice(old.length - tail)) {
    if (e.nodes.length) {
      anchor = e.nodes[0];
      break;
    }
  }
  const middle = parts.slice(head, parts.length - tail).map((p) => {
    const reuse = spare.get(p.key);
    return reuse && reuse.length ? reuse.shift() : { key: p.key, nodes: nodesOf(p.html) };
  });
  for (const list of spare.values()) for (const e of list) for (const n of e.nodes) n.remove();
  for (const e of middle) for (const n of e.nodes) article.insertBefore(n, anchor);
  const entries = [...old.slice(0, head), ...middle, ...old.slice(old.length - tail)];
  entries.forEach((e, i) => {
    e.line = parts[i].line;
    e.endLine = parts[i].endLine;
    mark(e);
  });
  state.entries = entries;
}
