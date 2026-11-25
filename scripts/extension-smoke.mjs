import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../web/package.json'));
const { chromium } = require('playwright');

const ext = join(here, '../extension/dist');
const shots = process.env.SHOTS;
const dark = process.env.DARK === '1';

const page = `<!doctype html><html><head><meta charset="utf-8"><style>
body { font: 16px/1.6 system-ui, sans-serif; max-width: 720px; margin: 24px auto; padding: 0 16px; background: ${dark ? '#262624' : '#faf9f5'}; color: ${dark ? '#e8e6dc' : '#1f1e1d'}; }
.code-box { border-radius: 8px; background: ${dark ? '#1f1e1d' : '#f0eee6'}; margin: 1em 0; }
.code-box .label { font-size: 12px; padding: 4px 12px; opacity: .7; }
pre { margin: 0; padding: 12px; overflow-x: auto; }
</style></head><body>
<div class="msg">
<p>The sum of the first n numbers:</p>
<div class="code-box"><div class="label">fn</div><pre><code class="language-fn">sum_(i=1)^n i = frac(n(n+1), 2)</code></pre></div>
<p>A matrix and a short text with inline math:</p>
<div class="code-box"><div class="label">functor</div><pre><code class="language-functor">set mat(fence: brack)

Let $A = mat(1, 2; 3, 4)$ and $x in R.bb^2$. Then $A x = b$ has one solution when $det A eq.not 0$.</code></pre></div>
<p>Inline: the hat is <code>fn: theta.hat</code> and the set is <code>fn: R.bb</code>. Normal code: <code>let x = 1</code>.</p>
<div class="code-box"><div class="label">python</div><pre><code class="language-python">print("not functor")</code></pre></div>
<div class="code-box" id="stream"><div class="label">fn</div><pre><code class="language-fn"></code></pre></div>
</div></body></html>`;

const fail = (msg) => {
  console.error('FAIL', msg);
  process.exit(1);
};

const ctx = await chromium.launchPersistentContext(mkdtempSync(join(tmpdir(), 'fnx-')), {
  headless: false,
  args: ['--headless=new', `--disable-extensions-except=${ext}`, `--load-extension=${ext}`],
  viewport: { width: 800, height: 900 },
  deviceScaleFactor: shots ? 2 : 1,
});
await ctx.route('https://claude.ai/**', (r) => r.fulfill({ contentType: 'text/html', body: page }));
const p = await ctx.newPage();
p.on('console', (m) => m.type() === 'error' && console.error('console:', m.text()));
await p.goto('https://claude.ai/chat/test');

const stream = 'integral_0^1 x^2 d x = frac(1, 3)';
for (let i = 1; i <= stream.length; i += 4) {
  await p.evaluate((t) => (document.querySelector('#stream code').textContent = t), stream.slice(0, i));
  await p.waitForTimeout(60);
}
await p.evaluate((t) => (document.querySelector('#stream code').textContent = t), stream);

await p.waitForFunction(() => document.querySelectorAll('.functor-host').length === 3, null, { timeout: 15000 }).catch(() => fail('expected 3 rendered blocks'));
await p.waitForFunction(
  () => [...document.querySelectorAll('.functor-host')].every((h) => h.shadowRoot.querySelector('.out svg.fn-math')),
  null,
  { timeout: 15000 },
).catch(() => fail('blocks have no math svg'));
await p.waitForFunction(() => document.querySelectorAll('.functor-inline svg').length === 2, null, { timeout: 15000 }).catch(() => fail('expected 2 inline math'));

await p.waitForFunction(
  (t) => document.querySelector('#stream .functor-host')?.shadowRoot.querySelector('svg.fn-math')?.getAttribute('aria-label') === t,
  stream,
  { timeout: 15000 },
).catch(() => fail('streamed block did not reach its final text'));

const info = await p.evaluate(() => {
  const hosts = [...document.querySelectorAll('.functor-host')];
  return {
    hidden: hosts.map((h) => getComputedStyle(h.previousElementSibling).display),
    diags: hosts.map((h) => h.shadowRoot.querySelector('.diag').textContent),
    stream: document.querySelector('#stream .functor-host').shadowRoot.querySelector('svg').getAttribute('aria-label'),
    python: !!document.querySelector('.language-python').closest('.code-box').querySelector('.functor-host'),
    normal: getComputedStyle([...document.querySelectorAll('code')].find((c) => c.textContent === 'let x = 1')).display,
  };
});
if (info.hidden.some((d) => d !== 'none')) fail(`source still visible: ${info.hidden}`);
if (info.diags.some(Boolean)) fail(`diagnostics: ${info.diags}`);
if (info.stream !== stream) fail(`stream not final: ${info.stream}`);
if (info.python) fail('python block was rendered');
if (info.normal === 'none') fail('normal inline code was hidden');

if (shots) await p.screenshot({ path: join(shots, dark ? 'extension-dark.png' : 'extension.png'), fullPage: true });

const host = p.locator('.functor-host').first();
await host.locator('button').click();
if ((await p.evaluate(() => getComputedStyle(document.querySelector('pre')).display)) === 'none') fail('show source did not work');
await host.locator('button').click();
if ((await p.evaluate(() => getComputedStyle(document.querySelector('pre')).display)) !== 'none') fail('show math did not work');

console.log('extension smoke ok', info);
await ctx.close();
