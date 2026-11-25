import { StreamLanguage, HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';
import { autocompletion } from '@codemirror/autocomplete';
import { linter } from '@codemirror/lint';

const DECL = /^(object|sym|let|set|use|kind|style)\b/;

const functorStream = {
  name: 'functor',
  startState: () => ({ math: false, display: false, code: false, front: false, line: 0 }),
  token(stream, s) {
    if (stream.sol()) {
      s.line++;
      if (s.line === 1 && stream.match(/^---\s*$/)) { s.front = true; return 'meta'; }
      if (s.front) {
        if (stream.match(/^---\s*$/)) { s.front = false; return 'meta'; }
        if (stream.match(/^[\w-]+(?=:)/)) return 'propertyName';
        stream.skipToEnd();
        return 'string';
      }
      if (stream.match(/^```/)) { s.code = !s.code; stream.skipToEnd(); return 'monospace'; }
      if (s.code) { stream.skipToEnd(); return 'monospace'; }
      if (!s.display && !s.math) {
        if (stream.match(/^#{1,6} /)) { stream.skipToEnd(); return 'heading'; }
        if (stream.match(/^:::/)) { stream.skipToEnd(); return 'keyword'; }
        if (stream.match(DECL)) return 'keyword';
      }
    }
    if (s.code) { stream.skipToEnd(); return 'monospace'; }
    if (s.display || s.math) {
      if (stream.match('$$')) { s.display = false; return 'processingInstruction'; }
      if (s.math && stream.eat('$')) { s.math = false; return 'processingInstruction'; }
      if (stream.match(/^[A-Za-z][A-Za-z0-9]*/)) return 'variableName';
      if (stream.match(/^\.[A-Za-z][A-Za-z0-9]*/)) return 'operator';
      if (stream.match(/^[0-9.]+/)) return 'number';
      if (stream.match(/^"[^"]*"?/)) return 'string';
      stream.next();
      return 'punctuation';
    }
    if (stream.match('$$')) { s.display = true; return 'processingInstruction'; }
    if (stream.eat('$')) { s.math = true; return 'processingInstruction'; }
    if (stream.match(/^\{#[^}]*\}/)) return 'labelName';
    if (stream.match(/^@[A-Za-z][\w:.-]*[\w]/)) return 'link';
    if (stream.match(/^`[^`]*`/)) return 'monospace';
    if (stream.match(/^\*\*[^*]+\*\*/)) return 'strong';
    if (stream.match(/^\{[A-Za-z_][\w-]*\}/)) return 'special';
    stream.next();
    return null;
  },
};

export const functorLanguage = StreamLanguage.define(functorStream);

export const functorHighlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: t.heading, fontWeight: '600', color: 'var(--hl-heading)' },
    { tag: t.keyword, color: 'var(--hl-keyword)', fontWeight: '600' },
    { tag: t.meta, color: 'var(--hl-muted)' },
    { tag: t.propertyName, color: 'var(--hl-keyword)' },
    { tag: t.processingInstruction, color: 'var(--hl-math)', fontWeight: '600' },
    { tag: t.variableName, color: 'var(--hl-math)' },
    { tag: t.operator, color: 'var(--hl-op)', fontWeight: '600' },
    { tag: t.number, color: 'var(--hl-number)' },
    { tag: t.string, color: 'var(--hl-string)' },
    { tag: t.labelName, color: 'var(--hl-label)' },
    { tag: t.link, color: 'var(--hl-label)', textDecoration: 'underline' },
    { tag: t.monospace, color: 'var(--hl-muted)' },
    { tag: t.strong, fontWeight: '700' },
    { tag: t.special(t.variableName), color: 'var(--hl-string)' },
  ]),
);

function inMath(state, pos) {
  const line = state.doc.lineAt(pos);
  const before = line.text.slice(0, pos - line.from);
  const dollars = (before.replace(/\\\$/g, '').match(/\$/g) || []).length;
  if (dollars % 2 === 1) return true;
  const text = state.doc.sliceString(0, pos);
  return (text.match(/\$\$/g) || []).length % 2 === 1;
}

function preview(fn, recipe) {
  return async () => {
    const svg = await fn.glyph(recipe);
    if (!svg) return null;
    const el = document.createElement('div');
    el.className = 'cm-glyph-preview';
    el.innerHTML = svg;
    const cap = document.createElement('code');
    cap.textContent = recipe;
    el.appendChild(cap);
    return el;
  };
}

export function functorCompletion(fn, info) {
  const names = info.names;
  const ops = info.ops;
  const blocks = info.blocks;
  return autocompletion({
    activateOnTyping: true,
    override: [
      (ctx) => {
        const line = ctx.state.doc.lineAt(ctx.pos);
        const before = line.text.slice(0, ctx.pos - line.from);
        const block = /^:::\s*(\w*)$/.exec(before);
        if (block) {
          return {
            from: ctx.pos - block[1].length,
            options: blocks.map((b) => ({ label: b, type: 'keyword' })),
          };
        }
        if (!inMath(ctx.state, ctx.pos)) return null;
        const op = /([A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z0-9]+)*)\.([A-Za-z0-9]*)$/.exec(before);
        if (op) {
          const base = op[1];
          return {
            from: ctx.pos - op[2].length,
            options: ops.map((o, i) => ({
              label: o.name,
              type: 'method',
              boost: Math.max(-99, 99 - 2 * i),
              detail: o.meaning.split(':')[0],
              info: async () => {
                const el = await preview(fn, `${base}.${o.name}`)();
                const m = document.createElement('div');
                m.className = 'cm-op-meaning';
                m.textContent = o.meaning;
                const box = document.createElement('div');
                box.appendChild(m);
                if (el) box.appendChild(el);
                return box;
              },
            })),
            validFor: /^[A-Za-z0-9]*$/,
          };
        }
        const word = ctx.matchBefore(/[A-Za-z][A-Za-z0-9]*/);
        if (!word || (word.from === word.to && !ctx.explicit)) return null;
        return {
          from: word.from,
          options: names.map((n) => ({
            label: n.name,
            type: n.kind === 'call' ? 'function' : n.kind === 'function' ? 'function' : n.kind === 'space' ? 'text' : 'variable',
            detail: n.kind,
            info: n.kind === 'call' || n.kind === 'space' ? undefined : preview(fn, n.name),
          })),
          validFor: /^[A-Za-z0-9]*$/,
        };
      },
    ],
  });
}

export function functorLinter(getResult) {
  return linter(
    (view) => {
      const out = getResult();
      if (!out) return [];
      const doc = view.state.doc;
      return out.diagnostics.map((d) => {
        const ln = doc.line(Math.min(Math.max(d.line, 1), doc.lines));
        const from = Math.min(ln.from + d.col - 1, ln.to);
        const rest = doc.sliceString(from, ln.to);
        const m = /^[\w.@:#-]+/.exec(rest);
        const to = Math.min(from + (m ? m[0].length : 1), ln.to);
        return {
          from,
          to: Math.max(to, from),
          severity: d.severity,
          message: d.hint ? `${d.message}\n${d.hint}` : d.message,
        };
      });
    },
    { delay: 0 },
  );
}
