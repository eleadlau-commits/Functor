const WORD = /[\p{L}\p{N}_]/u;
const SKIP = 'svg, .fn-num, .fn-env-name, .fn-eqnum, .fn-qed, .fn-env-sep';
const BLOCK = '[data-fn-line], [data-fn-rel]';

export function blockLines(el) {
  const top = el.closest('[data-fn-line]');
  if (!top) return null;
  const base = Number(top.dataset.fnLine);
  if (el !== top && el.dataset.fnRel) {
    const [a, b] = el.dataset.fnRel.split('-').map(Number);
    return { from: base + a, to: base + b };
  }
  return { from: base, to: Number(top.dataset.fnEndLine) };
}

function blockAt(root, line) {
  let best = null;
  let bestSize = Infinity;
  let after = null;
  let before = null;
  for (const el of root.querySelectorAll(BLOCK)) {
    const r = blockLines(el);
    if (!r) continue;
    if (r.from <= line && line <= r.to && r.to - r.from <= bestSize) {
      best = el;
      bestSize = r.to - r.from;
    } else if (r.from > line && !after) after = el;
    else if (r.to < line) before = el;
  }
  return best || after || before;
}

function sourceRange(doc, r) {
  const a = doc.line(Math.min(Math.max(r.from, 1), doc.lines));
  const b = doc.line(Math.min(Math.max(r.to, 1), doc.lines));
  return { from: a.from, to: b.to };
}

function maskMath(text) {
  return text.replace(/\$\$[\s\S]*?\$\$|\$[^$\n]*\$/g, (m) => m.replace(/[^\n]/g, ' '));
}

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function wordRe(word) {
  return new RegExp(`(?<![\\p{L}\\p{N}_])${escapeRe(word)}(?![\\p{L}\\p{N}_])`, 'gu');
}

function mathRe(label) {
  return new RegExp(label.split(/\s+/).filter(Boolean).map(escapeRe).join('\\s*'), 'g');
}

function nth(re, text, k) {
  let m;
  let last = null;
  let i = 0;
  re.lastIndex = 0;
  while ((m = re.exec(text))) {
    last = m;
    if (i++ === k) return m;
    if (m[0].length === 0) re.lastIndex++;
  }
  return last;
}

function count(re, text) {
  re.lastIndex = 0;
  return (text.match(re) || []).length;
}

function textNodes(el) {
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => (n.parentElement.closest(SKIP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  const out = [];
  while (walker.nextNode()) out.push(walker.currentNode);
  return out;
}

function wordAround(text, i) {
  let a = i;
  let b = i;
  while (a > 0 && WORD.test(text[a - 1])) a--;
  while (b < text.length && WORD.test(text[b])) b++;
  return a < b ? { word: text.slice(a, b), from: a, to: b } : null;
}

function caretAt(x, y) {
  if (document.caretPositionFromPoint) {
    const p = document.caretPositionFromPoint(x, y);
    return p && { node: p.offsetNode, offset: p.offset };
  }
  if (document.caretRangeFromPoint) {
    const r = document.caretRangeFromPoint(x, y);
    return r && { node: r.startContainer, offset: r.startOffset };
  }
  return null;
}

function mathAround(lineText, col) {
  const re = /\$\$?([^$]+)\$?\$?/g;
  let m;
  while ((m = re.exec(lineText))) {
    if (m.index <= col && col <= m.index + m[0].length) return { label: m[1].trim(), start: m.index };
  }
  return null;
}

function flash(el) {
  el.classList.remove('fn-sync-flash');
  void el.getBoundingClientRect();
  el.classList.add('fn-sync-flash');
  setTimeout(() => el.classList.remove('fn-sync-flash'), 1300);
}

function highlight(range) {
  if (typeof Highlight === 'undefined' || !CSS.highlights) return;
  CSS.highlights.set('fn-sync', new Highlight(range));
  clearTimeout(highlight.timer);
  highlight.timer = setTimeout(() => CSS.highlights.delete('fn-sync'), 1600);
}

export function createSync(view, previewEl) {
  function toSource(e) {
    const block = e.target.closest?.(BLOCK);
    const lines = block && blockLines(block);
    if (!lines) return false;
    const doc = view.state.doc;
    const span = sourceRange(doc, lines);
    const src = doc.sliceString(span.from, span.to);
    let sel = { anchor: span.from };
    const math = e.target.closest('svg.fn-math');
    const label = math?.getAttribute('aria-label');
    if (label) {
      const same = [...block.querySelectorAll('svg.fn-math')].filter((s) => s.getAttribute('aria-label') === label);
      const m = nth(mathRe(label), src, same.indexOf(math));
      if (m) sel = { anchor: span.from + m.index, head: span.from + m.index + m[0].length };
    } else {
      const caret = caretAt(e.clientX, e.clientY);
      const w = caret && caret.node.nodeType === Node.TEXT_NODE && wordAround(caret.node.data, caret.offset);
      if (w) {
        const re = wordRe(w.word);
        let k = 0;
        for (const n of textNodes(block)) {
          if (n === caret.node) {
            k += count(re, n.data.slice(0, w.from));
            break;
          }
          k += count(re, n.data);
        }
        const m = nth(re, maskMath(src), k);
        if (m) sel = { anchor: span.from + m.index, head: span.from + m.index + m[0].length };
      }
    }
    view.dispatch({ selection: sel, scrollIntoView: true });
    view.focus();
    return true;
  }

  function toPreview() {
    const state = view.state;
    const head = state.selection.main.head;
    const line = state.doc.lineAt(head);
    const block = blockAt(previewEl, line.number);
    if (!block) return false;
    block.scrollIntoView({ block: 'center', behavior: 'smooth' });
    const lines = blockLines(block);
    const span = sourceRange(state.doc, lines);
    const src = state.doc.sliceString(span.from, span.to);
    const math = mathAround(line.text, head - line.from);
    if (math) {
      const k = count(mathRe(math.label), src.slice(0, Math.max(line.from + math.start - span.from, 0)));
      const norm = math.label.split(/\s+/).join(' ');
      const same = [...block.querySelectorAll('svg.fn-math')].filter((s) => s.getAttribute('aria-label') === norm);
      const svg = same[Math.min(Math.max(k, 0), same.length - 1)];
      flash(svg || block);
      return true;
    }
    flash(block);
    const masked = maskMath(src);
    const at = head - span.from;
    const w = wordAround(masked, at) || wordAround(masked, at - 1);
    if (!w) return true;
    const re = wordRe(w.word);
    let k = count(re, masked.slice(0, w.from));
    for (const n of textNodes(block)) {
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(n.data))) {
        if (k-- === 0) {
          const r = document.createRange();
          r.setStart(n, m.index);
          r.setEnd(n, m.index + m[0].length);
          highlight(r);
          return true;
        }
      }
    }
    return true;
  }

  function mark() {
    for (const el of previewEl.querySelectorAll('.fn-sync-here')) el.classList.remove('fn-sync-here');
    const line = view.state.doc.lineAt(view.state.selection.main.head).number;
    const block = blockAt(previewEl, line);
    const r = block && blockLines(block);
    if (r && r.from <= line && line <= r.to) block.classList.add('fn-sync-here');
  }

  previewEl.addEventListener('dblclick', (e) => {
    if (e.target.closest('a')) return;
    toSource(e);
  });

  return { toSource, toPreview, mark };
}
