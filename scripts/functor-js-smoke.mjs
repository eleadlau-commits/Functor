import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../web/package.json'));
const { chromium } = require('playwright');

const root = join(here, '../js');
const shots = process.env.SHOTS;
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const STRICT = "default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src data:; img-src data:; connect-src 'none'";

const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  let file = join(root, decodeURIComponent(url.pathname));
  if (existsSync(file) && statSync(file).isDirectory()) file = join(file, 'index.html');
  if (!file.startsWith(root) || !existsSync(file)) {
    res.writeHead(404).end();
    return;
  }
  const head = { 'content-type': types[extname(file)] || 'application/octet-stream' };
  if (url.searchParams.has('strict')) head['content-security-policy'] = STRICT;
  res.writeHead(200, head).end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, ok));
const base = `http://127.0.0.1:${server.address().port}`;

const fail = (msg) => {
  console.error('FAIL', msg);
  process.exit(1);
};

const browser = await chromium.launch();

async function check(name, query, scheme) {
  const page = await browser.newPage({ viewport: { width: 820, height: 1000 }, deviceScaleFactor: shots ? 2 : 1, colorScheme: scheme });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && !/Content Security Policy|Failed to load resource/.test(m.text()) && errors.push(m.text()));
  await page.goto(`${base}/demo/${query}`);
  await page
    .waitForFunction(
      () => {
        const inl = [...document.querySelectorAll('fn-math:not([display])')];
        const blk = [...document.querySelectorAll('fn-math[display], fn-doc, .functor-host')];
        return inl.length && inl.every((e) => e.shadowRoot?.querySelector('svg, .src')) && blk.every((e) => e.shadowRoot?.querySelector('.out svg')) && document.querySelector('#api svg');
      },
      null,
      { timeout: 20000 },
    )
    .catch(() => fail(`${name}: math did not render`));
  const info = await page.evaluate(() => ({
    inline: document.querySelectorAll('fn-math:not([display])').length,
    svgs: [...document.querySelectorAll('fn-math, fn-doc, .functor-host, .functor-inline')].filter((e) => e.shadowRoot?.querySelector('svg')).length,
    eps: document.querySelector('fn-math').shadowRoot.querySelectorAll('svg').length,
    errorShown: [...document.querySelectorAll('fn-math')].filter((e) => e.shadowRoot?.querySelector('.src')).map((e) => e.textContent),
    codeHidden: getComputedStyle(document.querySelector('pre code.language-fn').parentElement).display,
    inlineCode: !!document.querySelector('.functor-inline')?.shadowRoot?.querySelector('svg'),
    fonts: [...document.fonts].filter((f) => f.family.includes('Functor')).length,
    ink: getComputedStyle(document.querySelector('fn-doc')).color,
    wasmScript: !!globalThis.__functorWasm,
  }));
  if (info.eps < 2) fail(`${name}: inline math lost parts: ${info.eps} svgs`);
  if (info.errorShown.length !== 1) fail(`${name}: expected one error, got ${info.errorShown}`);
  if (info.codeHidden !== 'none') fail(`${name}: fn code block still shown`);
  if (!info.inlineCode) fail(`${name}: fn: inline code not rendered`);
  await page.click('#more');
  await page.waitForFunction(() => document.querySelector('#live').shadowRoot.querySelector('svg')?.getAttribute('aria-label') === 'x^2', null, { timeout: 5000 }).catch(() => fail(`${name}: live change not rendered`));
  if (errors.length) fail(`${name}: ${errors.join('\n')}`);
  if (shots) await page.screenshot({ path: join(shots, `functor-js-${name}.png`), fullPage: true });
  console.log(name, 'ok', info);
  await page.close();
  return info;
}

await check('light', '', 'light');
const dark = await check('dark', '', 'dark');
if (!/2[0-9]{2}/.test(dark.ink)) fail(`dark: text colour not light: ${dark.ink}`);
const strict = await check('strict', '?strict', 'light');
if (!strict.wasmScript) fail('strict: the script fallback for the wasm was not used');

await browser.close();
server.close();
console.log('functor.js smoke ok');
