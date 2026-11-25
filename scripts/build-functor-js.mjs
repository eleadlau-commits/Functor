import { execSync } from 'node:child_process';
import { cpSync, mkdirSync, rmSync, existsSync, readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'js');
const out = join(src, 'dist');
const wasm = join(root, 'target/wasm32-unknown-unknown/release/functor_wasm.wasm');

if (!process.argv.includes('--no-cargo')) {
  execSync('cargo build --release -p functor-wasm --target wasm32-unknown-unknown', { cwd: root, stdio: 'inherit' });
}
if (!existsSync(wasm)) throw new Error(`missing ${wasm}`);

const version = readFileSync(join(root, 'Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
const fonts = {};
for (const f of readdirSync(join(root, 'web/src/fonts'))) {
  if (f.endsWith('.woff2')) fonts[f.replace(/\.woff2$/, '')] = readFileSync(join(root, 'web/src/fonts', f)).toString('base64');
}
const css = readFileSync(join(root, 'web/src/doc.css'), 'utf8');
const core = readFileSync(join(src, 'src/core.js'), 'utf8');
const web = readFileSync(join(src, 'src/web.js'), 'utf8')
  .replace('__FUNCTOR_CSS__', () => JSON.stringify(css))
  .replace('__FUNCTOR_VERSION__', () => JSON.stringify(version))
  .replaceAll('__FUNCTOR_FONTS__', 'fnFonts');
const bytes = readFileSync(wasm);

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
writeFileSync(
  join(out, 'functor.js'),
  `/*! functor.js ${version} | MIT | Latin Modern fonts: GUST Font License */\n(() => {\nconst fnFonts = ${JSON.stringify(fonts)};\n${core}\n${web}})();\n`,
);
cpSync(wasm, join(out, 'functor.wasm'));
writeFileSync(join(out, 'functor-wasm.js'), `globalThis.__functorWasm=${JSON.stringify(bytes.toString('base64'))};\n`);
cpSync(join(src, 'README.md'), join(out, 'README.md'));
cpSync(join(root, 'LICENSE'), join(out, 'LICENSE'));
cpSync(join(root, 'web/src/fonts/GUST-FONT-LICENSE.txt'), join(out, 'GUST-FONT-LICENSE.txt'));
const pkg = JSON.parse(readFileSync(join(src, 'package.json'), 'utf8'));
pkg.version = version;
writeFileSync(join(out, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');
console.log(`functor.js ${version} built in ${out}`);
