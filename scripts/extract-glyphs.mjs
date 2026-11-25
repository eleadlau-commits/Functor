import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../web/package.json'));
const pkgDir = dirname(require.resolve('@mathjax/mathjax-newcm-font/package.json'));
const svgDir = join(pkgDir, 'mjs/svg');

export const CAL_BASE = 0xf0000;

const ranges = [
  [0x20, 0x7e],
  [0xa0, 0xff],
  [0x131, 0x131],
  [0x19b, 0x19b],
  [0x237, 0x237],
  [0x2c6, 0x2dd],
  [0x391, 0x3f6],
  [0x2016, 0x2025],
  [0x2027, 0x2060],
  [0x20d0, 0x20ff],
  [0x2100, 0x214f],
  [0x2190, 0x23ff],
  [0x25a0, 0x25ff],
  [0x2605, 0x2606],
  [0x2660, 0x266f],
  [0x2713, 0x2713],
  [0x2717, 0x2717],
  [0x27c0, 0x27ff],
  [0x2900, 0x2aff],
  [0x2b30, 0x2b4f],
  [0x1d400, 0x1d7ff],
];
const wanted = (cp) => ranges.some(([a, b]) => cp >= a && cp <= b);

const entry = /^\s+0x([0-9A-F]+):\s*\[([-\d.]+),\s*([-\d.]+),\s*([-\d.]+)(?:,\s*\{([^\n]*)\})?\]/gm;

function entries(text) {
  const out = [];
  for (const m of text.matchAll(entry)) {
    const p = /(?:^|[\s,])p:\s*'([^']*)'/.exec(m[5] || '');
    out.push({ cp: parseInt(m[1], 16), h: +m[2], d: +m[3], w: +m[4], p: p ? p[1] : '' });
  }
  return out;
}

function sections(text) {
  const heads = [...text.matchAll(/^ {4}'([\w-]+)':\s*\{/gm)];
  return heads.map((h, i) => ({ name: h[1], body: text.slice(h.index, i + 1 < heads.length ? heads[i + 1].index : text.length) }));
}

const sources = [];
sources.push({ key: 'n', text: readFileSync(join(svgDir, 'normal.js'), 'utf8') });
sources.push({ key: 'n', text: readFileSync(join(svgDir, 'bbold.js'), 'utf8') });
sources.push({ key: 'L', text: readFileSync(join(svgDir, 'largeop.js'), 'utf8') });
const dyn = join(svgDir, 'dynamic');
for (const f of readdirSync(dyn).filter((f) => f.endsWith('.js')).sort()) {
  for (const s of sections(readFileSync(join(dyn, f), 'utf8'))) {
    if (s.name === 'normal') sources.push({ key: 'n', text: s.body });
    else if (s.name === 'largeop' || s.name === '-largeop') sources.push({ key: 'L', text: s.body });
    else if (s.name === '-tex-calligraphic') sources.push({ key: 'c', text: s.body });
  }
}

const lines = [];
const seen = new Set();
const r = (x) => Math.round(x * 1000);
for (const { key, text } of sources) {
  for (const e of entries(text)) {
    let cp = e.cp;
    let v = key;
    if (key === 'c') {
      if (cp < 0x41 || cp > 0x5a) continue;
      cp += CAL_BASE;
      v = 'n';
    } else if (!wanted(cp)) continue;
    const p = e.p && !/^[A-Za-z]/.test(e.p) ? 'M' + e.p : e.p;
    if (!p && cp !== 0x20) continue;
    if (seen.has(v + cp)) continue;
    seen.add(v + cp);
    lines.push([v, cp.toString(16), r(e.h), r(e.d), r(e.w), p].join('\t'));
  }
}

const out = join(here, '../crates/functor-symbols/data/glyphs.tsv');
writeFileSync(out, lines.join('\n') + '\n');
console.log(`wrote ${lines.length} glyphs to ${out}`);
