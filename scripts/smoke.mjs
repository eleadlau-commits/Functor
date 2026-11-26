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

  const spot = (sel, word) => page.evaluate(([sel, word]) => {
    const el = document.querySelector(sel);
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const n = walker.currentNode;
      const i = n.data.indexOf(word);
      if (i < 0) continue;
      const r = document.createRange();
      r.setStart(n, i + 1);
      r.setEnd(n, i + 2);
      r.startContainer.parentElement.scrollIntoView({ block: 'center' });
      const b = r.getBoundingClientRect();
      return { x: b.left + b.width / 2, y: b.top + b.height / 2 };
    }
    return null;
  }, [sel, word]);
  const lineOf = () => page.evaluate(() => {
    const active = document.querySelector('.cm-activeLine');
    return { text: document.getSelection().toString(), line: active?.textContent ?? '' };
  });
  let at = await spot('#preview .fn-definition [data-fn-rel]', 'related');
  if (!at) fail('no word "related" in a nested block of the preview');
  else {
    await page.mouse.dblclick(at.x, at.y);
    await page.waitForTimeout(100);
    const got = await lineOf();
    console.log('preview to source:', got.text, '|', got.line.slice(0, 40));
    if (got.text !== 'related' || !got.line.includes('A related value')) fail('double-click in the preview did not select the word in the source');
  }
  const mathBox = await page.locator('#preview svg.fn-math[aria-label="x in.not A"]').boundingBox();
  if (mathBox) {
    await page.mouse.dblclick(mathBox.x + mathBox.width / 2, mathBox.y + mathBox.height / 2);
    await page.waitForTimeout(100);
    const got = await lineOf();
    console.log('math to source:', got.text);
    if (got.text !== 'x in.not A') fail('double-click on math did not select its source');
  } else fail('no math x in.not A in the preview');
  at = await spot('#preview .fn-definition [data-fn-rel]', 'related');
  if (at) await page.mouse.dblclick(at.x, at.y);
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Alt+Enter');
  await page.waitForTimeout(100);
  const hl = await page.evaluate(() => {
    const h = window.CSS?.highlights?.get('fn-sync');
    const r = h && [...h][0];
    return { word: r ? r.toString() : null, here: document.querySelector('.fn-sync-here')?.textContent.slice(0, 30) };
  });
  console.log('source to preview:', hl);
  if (hl.word !== 'related') fail('Alt+Enter did not mark the word in the preview');
  if (!hl.here || !hl.here.includes('An estimate')) fail('the preview does not mark the block at the cursor');
  if (shots) await page.screenshot({ path: `${shots}/site-locate.png` });

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
