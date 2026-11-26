import nspell from 'nspell';
import extra from './words.txt?raw';

let spell = null;
let ready = null;

async function text(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`The dictionary did not load (${r.status}).`);
  return r.text();
}

async function load(dict, personal) {
  const [aff, dic] = await Promise.all([text(dict.aff), text(dict.dic)]);
  const s = nspell(aff, dic);
  for (const w of extra.split('\n')) if (w.trim()) s.add(w.trim());
  for (const w of personal) s.add(w);
  return s;
}

const plain = (w) => w.replace(/’/g, "'");

function correct(w) {
  const x = plain(w);
  if (spell.correct(x)) return true;
  if (/'s$/.test(x) && spell.correct(x.slice(0, -2))) return true;
  return false;
}

function distance(a, b) {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  }
  return d[a.length][b.length];
}

const sorted = (w) => [...w].sort().join('');

function inside(a, b) {
  let i = 0;
  for (const c of b) if (c === a[i]) i++;
  return i === a.length;
}

export function rank(word, list) {
  const w = word.toLowerCase();
  const lower = w === word;
  return list
    .map((s, i) => {
      const t = s.toLowerCase();
      return { s, key: [distance(w, t), lower && s !== t ? 1 : 0, sorted(w) === sorted(t) ? 0 : 1, inside(w, t) ? 0 : 1, i] };
    })
    .sort((x, y) => {
      for (let k = 0; k < x.key.length; k++) if (x.key[k] !== y.key[k]) return x.key[k] - y.key[k];
      return 0;
    })
    .map((x) => x.s);
}

self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    if (type === 'load') {
      spell = null;
      ready = load(data.dict, data.personal);
      spell = await ready;
      self.postMessage({ id, ok: true });
      return;
    }
    if (!spell) spell = await ready;
    if (type === 'check') self.postMessage({ id, bad: data.words.filter((w) => !correct(w)) });
    else if (type === 'suggest') self.postMessage({ id, list: rank(plain(data.word), spell.suggest(plain(data.word))).slice(0, 6) });
    else if (type === 'add') {
      spell.add(plain(data.word));
      self.postMessage({ id, ok: true });
    }
  } catch (e) {
    self.postMessage({ id, error: String(e && e.message ? e.message : e) });
  }
};
