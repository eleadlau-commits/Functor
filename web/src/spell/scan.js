const DECL = /^(object|sym|let|set|use|kind|style)\b/;
const FENCE = /^---\s*$/;
const WORD = /\p{L}+(?:['’]\p{L}+)*/uy;
const LETTER = /\p{L}/u;
const UPPER = /\p{Lu}/u;

function keep(line, from, to, word) {
  if (word.length < 2) return false;
  if (UPPER.test(word.slice(1))) return false;
  const before = line[from - 1] ?? '';
  const after = line[to] ?? '';
  if (/[\d_]/.test(before) || /[\d_]/.test(after)) return false;
  if (before === '.' && LETTER.test(line[from - 2] ?? '')) return false;
  if (after === '.' && LETTER.test(line[to + 1] ?? '')) return false;
  return true;
}

function close(line, i, ch) {
  const j = line.indexOf(ch, i + 1);
  return j < 0 ? i + 1 : j + 1;
}

function scanLine(line, base, st, out) {
  let i = 0;
  while (i < line.length) {
    const c = line[i];
    if (st.display || st.math) {
      if (line.startsWith('$$', i)) { st.display = false; st.math = false; i += 2; }
      else if (st.math && c === '$') { st.math = false; i++; }
      else if (c === '"') i = close(line, i, '"');
      else if (c === '\\') i += 2;
      else i++;
      continue;
    }
    if (c === '\\') { i += 2; continue; }
    if (line.startsWith('$$', i)) { st.display = true; i += 2; continue; }
    if (c === '$') { st.math = true; i++; continue; }
    if (c === '`') { i = close(line, i, '`'); continue; }
    if (c === '{') { i = close(line, i, '}'); continue; }
    if (c === '@' && LETTER.test(line[i + 1] ?? '')) {
      i++;
      while (i < line.length && /[\p{L}\d_:.-]/u.test(line[i])) i++;
      continue;
    }
    if (c === ']' && line[i + 1] === '(') { i = close(line, i + 1, ')'); continue; }
    if (c === '<') {
      const m = /^<[^>\s]+>/.exec(line.slice(i));
      if (m) { i += m[0].length; continue; }
    }
    if (/^(https?:\/\/|www\.)/.test(line.slice(i, i + 8))) {
      while (i < line.length && !/\s/.test(line[i])) i++;
      continue;
    }
    WORD.lastIndex = i;
    const m = WORD.exec(line);
    if (m) {
      const to = i + m[0].length;
      if (keep(line, i, to, m[0])) out.push({ from: base + i, to: base + to, word: m[0] });
      i = to;
      continue;
    }
    i++;
  }
}

export function proseWords(text) {
  const out = [];
  const st = { math: false, display: false };
  let code = false;
  let front = false;
  let pos = 0;
  const lines = text.split('\n');
  for (let n = 0; n < lines.length; n++) {
    const line = lines[n];
    const base = pos;
    pos += line.length + 1;
    if (n === 0 && FENCE.test(line)) { front = true; continue; }
    if (front) {
      if (FENCE.test(line)) front = false;
      else {
        const m = /^(title|subtitle|abstract):\s*/.exec(line);
        if (m) scanLine(line.slice(m[0].length), base + m[0].length, { math: false, display: false }, out);
      }
      continue;
    }
    if (line.startsWith('```')) { code = !code; continue; }
    if (code) continue;
    if (!st.math && !st.display) {
      if (DECL.test(line)) continue;
      if (line.startsWith(':::')) {
        const t = /title="((?:[^"\\]|\\.)*)"/.exec(line);
        if (t) {
          const at = t.index + t[0].indexOf('"') + 1;
          scanLine(t[1], base + at, { math: false, display: false }, out);
        }
        continue;
      }
    }
    if (st.math && !line.trim()) st.math = false;
    scanLine(line, base, st, out);
  }
  return out;
}
