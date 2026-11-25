import { execSync } from 'node:child_process';
import { cpSync, mkdirSync, rmSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'extension');
const out = join(src, 'dist');
const wasm = join(root, 'target/wasm32-unknown-unknown/release/functor_wasm.wasm');

if (!process.argv.includes('--no-cargo')) {
  execSync('cargo build --release -p functor-wasm --target wasm32-unknown-unknown', { cwd: root, stdio: 'inherit' });
}
if (!existsSync(wasm)) throw new Error(`missing ${wasm}`);

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'fonts'), { recursive: true });
for (const f of ['manifest.json', 'background.js', 'content.js', 'icons']) cpSync(join(src, f), join(out, f), { recursive: true });
cpSync(join(root, 'web/src/doc.css'), join(out, 'doc.css'));
cpSync(join(root, 'web/src/fonts'), join(out, 'fonts'), { recursive: true });
cpSync(wasm, join(out, 'functor.wasm'));
cpSync(join(root, 'LICENSE'), join(out, 'LICENSE'));
console.log(`extension built in ${out}`);
