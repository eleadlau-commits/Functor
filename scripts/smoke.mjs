import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../web/package.json'));
const { chromium } = require('playwright');

const root = join(here, '../web/dist');
const shots = process.env.SHOTS;
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.svg': 'image/svg+xml' };

const server = createServer(async (req, res) => {
  const path = join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/\/$/, '/index.html'));
  try {
    const body = await readFile(path);
    res.writeHead(200, { 'content-type': types[extname(path)] || 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end();
  }
});
await new Promise((r) => server.listen(0, r));
const url = `http://127.0.0.1:${server.address().port}/`;

const fail = (msg) => {
  console.error('FAIL', msg);
  process.exitCode = 1;
};

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(url);
  await page.waitForSelector('.status.ok', { timeout: 10000 });
  const status = await page.textContent('#status');
  console.log('status:', status);
  const svgs = await page.locator('#preview svg.fn-math').count();
  if (svgs < 20) fail(`only ${svgs} math SVGs in the preview`);
  if (shots) await page.screenshot({ path: `${shots}/site.png` });

  await page.click('.cm-content');
  await page.keyboard.press('Control+End');
  await page.keyboard.type('\n\nBad $alpah + x.circel$ and @eq:nope.\n');
  await page.waitForSelector('.status.bad', { timeout: 5000 });
  const items = await page.locator('#list li').allTextContents();
  console.log('problems:', items);
  if (!items.some((t) => t.includes('Did you mean `alpha`?'))) fail('no suggestion for alpah');
  if (!items.some((t) => t.includes('Did you mean `circle`?'))) fail('no suggestion for circel');
  if (!items.some((t) => t.includes('eq:nope'))) fail('no missing label error');

  await page.keyboard.type('$theta.ha');
  await page.waitForSelector('.cm-tooltip-autocomplete', { timeout: 5000 });
  await page.waitForSelector('.cm-completionInfo svg', { timeout: 5000 });
  const label = await page.textContent('.cm-tooltip-autocomplete li[aria-selected] .cm-completionLabel');
  console.log('completion:', label);
  if (label !== 'hat') fail(`first completion is ${label}`);
  if (shots) await page.screenshot({ path: `${shots}/site-complete.png` });

  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+a');
  await page.keyboard.type('---\nfunctor: 1\n---\n\nobject vec: v\n\n$$\nnorm(v) = (v).vbar.x2\n$$\n\nVector $v$');
  await page.keyboard.press('ArrowLeft');
  await page.waitForSelector('#inspector:not([hidden]) .insp-layers', { timeout: 5000 });
  const layers = await page.locator('#inspector .insp-layers li').allTextContents();
  console.log('inspector:', layers);
  if (layers.length !== 4 || !layers[0].includes('object vec: v') || !layers[2].includes('v.arrow')) fail('inspector does not show the four layers of v');
  const notes = await page.locator('#list li').allTextContents();
  if (!notes.some((t) => t.includes('Write `norm(v)`.'))) fail('no notation warning for (v).vbar.x2');
  if (shots) await page.screenshot({ path: `${shots}/site-inspector.png` });
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('End');
  for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowLeft');
  await page.waitForFunction(() => document.querySelector('#inspector .insp-source')?.textContent === '(v).vbar.x2', null, { timeout: 5000 });
  if (shots) await page.screenshot({ path: `${shots}/site-inspector-hand.png` });

  await page.click('#tabs .tab-add');
  await page.waitForFunction(() => document.querySelectorAll('#tabs .tab').length === 2);
  await page.fill('#name', 'notes.fn');
  await page.press('#name', 'Enter');
  await page.click('#tabs .tab:nth-child(1)');
  if (!(await page.textContent('.cm-content')).includes('object vec: v')) fail('switching tabs lost the first text');
  await page.reload();
  await page.waitForSelector('.status', { timeout: 10000 });
  const tabNames = await page.locator('#tabs .tab-name').allTextContents();
  console.log('tabs after reload:', tabNames);
  if (tabNames.length !== 2 || tabNames[1] !== 'notes.fn') fail('the website did not keep its tabs');

  await page.click('#tabs .tab-add');
  await page.click('.cm-content');
  await page.keyboard.press('Control+a');
  await page.keyboard.type('---\nfunctor: 1\ntitle: Teh title\n---\n\nobject recieve: v\n\nThe teh word $alpah + recieve$ and `teh` and @eq:teh {#sec:teh} and teh.hat.\n');
  await page.waitForSelector('.cm-misspelled', { timeout: 10000 });
  const wrong = await page.locator('.cm-misspelled').allTextContents();
  console.log('misspelled:', wrong);
  if (JSON.stringify(wrong) !== '["Teh","teh"]') fail(`spell check marks ${JSON.stringify(wrong)}, not only the prose words`);
  await page.locator('.cm-misspelled').nth(1).click({ button: 'right' });
  await page.waitForSelector('.cm-spell-fix', { timeout: 10000 });
  const fixes = await page.locator('.cm-spell-fix').allTextContents();
  console.log('suggestions:', fixes);
  if (!fixes.includes('the')) fail('no suggestion "the" for teh');
  if (shots) await page.screenshot({ path: `${shots}/site-spell-menu.png` });
  await page.locator('.cm-spell-fix', { hasText: /^the$/ }).click();
  await page.waitForFunction(() => document.querySelector('.cm-content').textContent.includes('The the word'), null, { timeout: 5000 });
  await page.waitForFunction(() => document.querySelectorAll('.cm-misspelled').length === 1, null, { timeout: 5000 });
  await page.keyboard.press('Control+End');
  await page.keyboard.type('A zorbly word.');
  await page.waitForFunction(() => document.querySelectorAll('.cm-misspelled').length === 2, null, { timeout: 5000 });
  await page.locator('.cm-misspelled').last().click({ button: 'right' });
  await page.click('.cm-spell-add');
  await page.waitForFunction(() => document.querySelectorAll('.cm-misspelled').length === 1, null, { timeout: 5000 });
  await page.click('#spell');
  const mine = await page.locator('.spell-words li span').allTextContents();
  if (!mine.includes('zorbly')) fail('zorbly is not in the word list');
  if (shots) await page.screenshot({ path: `${shots}/site-spell-panel.png` });
  await page.click('.spell-words button');
  await page.waitForFunction(() => document.querySelectorAll('.cm-misspelled').length === 2, null, { timeout: 5000 });
  await page.keyboard.press('Escape');

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await mobile.goto(url);
  await mobile.waitForSelector('.status', { timeout: 10000 });
  const overflow = await mobile.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  if (overflow) fail('horizontal scroll on phone width');
  if (shots) await mobile.screenshot({ path: `${shots}/site-mobile.png` });

  if (errors.length) fail(`page errors: ${errors.join('; ')}`);
  if (!process.exitCode) console.log('smoke test passed');
} finally {
  await browser.close();
  server.close();
}
