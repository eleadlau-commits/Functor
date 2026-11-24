import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../web/package.json'));
const pkgDir = dirname(require.resolve('@mathjax/mathjax-newcm-font/package.json'));

const variants = [
  ['n', 'normal'],
  ['n', 'bbold'],
  ['L', 'largeop'],
];

const ranges = [
  [0x20, 0x7e],
  [0xa0, 0xff],
  [0x2c6, 0x2dd],
  [0x391, 0x3f6],
  [0x2032, 0x2034],
  [0x20d0, 0x20ef],
  [0x2100, 0x214f],
  [0x2190, 0x21ff],
  [0x2200, 0x22ff],
  [0x2300, 0x23ff],
  [0x25a0, 0x25ff],
  [0x27c0, 0x27ff],
  [0x2980, 0x29ff],
  [0x2a00, 0x2aff],
  [0x1d400, 0x1d7ff],
];

const wanted = (cp) => ranges.some(([a, b]) => cp >= a && cp <= b);
const lines = [];
const seen = new Set();
for (const [key, file] of variants) {
  const mod = await import(pathToFileURL(join(pkgDir, 'mjs/svg', `${file}.js`)).href);
  const table = Object.values(mod)[0];
  for (const [cpText, entry] of Object.entries(table)) {
    const cp = Number(cpText);
    if (!wanted(cp) || !Array.isArray(entry)) continue;
    const [h, d, w, opts] = entry;
    const raw = opts && typeof opts.p === 'string' ? opts.p : '';
    const p = raw && !/^[A-Za-z]/.test(raw) ? 'M' + raw : raw;
    if (!p && cp !== 0x20) continue;
    if (seen.has(key + cp)) continue;
    seen.add(key + cp);
    const r = (x) => Math.round(x * 1000);
    lines.push([key, cp.toString(16), r(h), r(d), r(w), p].join('\t'));
  }
}

const out = join(here, '../crates/functor-core/data/glyphs.tsv');
writeFileSync(out, lines.join('\n') + '\n');
console.log(`wrote ${lines.length} glyphs to ${out}`);
