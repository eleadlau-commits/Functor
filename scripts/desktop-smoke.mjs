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

const shell = () => {
  const disk = JSON.parse(sessionStorage.getItem('mock.disk') || 'null') || {
    '/notes/groups.fn': '---\nfunctor: 1\ntitle: Groups\n---\n\n# Groups\n\nA group $G$ has $g h in G$ for all $g, h in G$.\n\n$$\n|G| = |Z(G)| + sum_(i = 1)^k [G : C_G (x_i)]\n$$\n',
    '/notes/limits.fn': '---\nfunctor: 1\ntitle: Limits\n---\n\n# Limits\n\n$$\nlim_(n to infty) (1 + frac(1, n))^n = e\n$$\n',
  };
  const keep = () => sessionStorage.setItem('mock.disk', JSON.stringify(disk));
  const doc = (path) => ({ path, name: path.split('/').pop(), text: disk[path] });
  const calls = [];
  const callbacks = new Map();
  const listeners = new Map();
  let next = 1;
  let wasm = null;
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const compile = async (src, width, parts, reset) => {
    if (!wasm) wasm = (await WebAssembly.instantiate(await (await fetch('/functor.wasm')).arrayBuffer(), {})).instance.exports;
    const data = enc.encode(src);
    const ptr = wasm.alloc(data.length);
    new Uint8Array(wasm.memory.buffer, ptr, data.length).set(data);
    if (reset) wasm.session_reset();
    const len = parts ? wasm.session_compile(ptr, data.length, width || 0) : wasm.compile_width(ptr, data.length, width || 0);
    wasm.dealloc(ptr, data.length);
    const json = dec.decode(new Uint8Array(wasm.memory.buffer, wasm.result_ptr(), len));
    return '{"nativeMs":0.5,' + json.slice(1);
  };
  window.__mock = { calls, disk, discard: false, pick: ['/notes/groups.fn', '/notes/limits.fn'] };
  window.__mock.emit = (event, payload) => (listeners.get(event) || []).forEach((id) => callbacks.get(id)?.({ event, id, payload }));
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback(cb) {
      const id = next++;
      callbacks.set(id, cb);
      return id;
    },
    async invoke(cmd, args = {}) {
      calls.push(cmd);
      switch (cmd) {
        case 'compile': return compile(args.src, args.width);
        case 'compile_parts': return compile(args.src, args.width, true, args.reset);
        case 'symbols': {
          if (!wasm) await compile('', 0);
          const len = wasm.symbols();
          return dec.decode(new Uint8Array(wasm.memory.buffer, wasm.result_ptr(), len));
        }
        case 'glyph': return '';
        case 'initial_file': return null;
        case 'load_file': return disk[args.path] != null ? doc(args.path) : null;
        case 'open_file': return window.__mock.pick.map(doc);
        case 'save_file': disk[args.path] = args.text; keep(); return doc(args.path);
        case 'close_file': return null;
        case 'ask_discard': return window.__mock.discard;
        case 'plugin:event|listen':
          if (!listeners.has(args.event)) listeners.set(args.event, []);
          listeners.get(args.event).push(args.handler);
          return args.handler;
        default: return null;
      }
    },
  };
};

const names = (page) => page.locator('#tabs .tab .tab-name').allTextContents();
const activeName = (page) => page.textContent('#tabs .tab[aria-selected="true"] .tab-name');
const editorText = (page) => page.evaluate(() => document.querySelector('.cm-content').innerText);

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(shell);
  await page.goto(url);
  await page.waitForSelector('.status.ok', { timeout: 10000 });

  if ((await names(page)).join() !== 'sample.fn') fail(`first start tabs: ${await names(page)}`);

  await page.click('.cm-content');
  await page.keyboard.press('Control+t');
  if ((await names(page)).join() !== 'sample.fn,untitled.fn') fail(`after Ctrl+T: ${await names(page)}`);
  if (!(await editorText(page)).includes('Write $x.hat$ here.')) fail('new tab has no blank document');
  await page.keyboard.press('Control+End');
  await page.keyboard.type('\nA draft line with $x^2$.');
  await page.waitForSelector('#tabs .tab.dirty', { timeout: 3000 });

  await page.keyboard.press('Control+o');
  await page.waitForFunction(() => document.querySelectorAll('#tabs .tab').length === 4);
  if ((await names(page)).join() !== 'sample.fn,untitled.fn,groups.fn,limits.fn') fail(`after open: ${await names(page)}`);
  if ((await activeName(page)) !== 'limits.fn') fail('the last opened file is not active');
  const title = await page.title();
  if (title !== 'limits.fn · Functor') fail(`title is ${title}`);

  await page.keyboard.press('Control+1');
  if ((await activeName(page)) !== 'sample.fn') fail('Ctrl+1 does not show the first tab');
  await page.keyboard.press('Control+Shift+Tab');
  if ((await activeName(page)) !== 'limits.fn') fail('Ctrl+Shift+Tab does not wrap to the last tab');
  await page.keyboard.press('Control+Tab');
  await page.keyboard.press('Control+Tab');
  if ((await activeName(page)) !== 'untitled.fn') fail('Ctrl+Tab does not go to the next tab');
  if (!(await editorText(page)).includes('A draft line')) fail('the untitled tab lost its text');

  await page.evaluate(() => {
    window.__mock.disk['/notes/groups.fn'] = window.__mock.disk['/notes/groups.fn'].replace('# Groups', '# Finite groups');
    sessionStorage.setItem('mock.disk', JSON.stringify(window.__mock.disk));
    window.__mock.emit('file-changed', { path: '/notes/groups.fn', name: 'groups.fn', text: window.__mock.disk['/notes/groups.fn'] });
  });
  await page.click('#tabs .tab:nth-child(3)');
  if (!(await editorText(page)).includes('# Finite groups')) fail('an outside change did not reach a tab in the background');

  await page.click('.cm-content');
  await page.keyboard.press('Control+End');
  await page.keyboard.type('\nEvery subgroup of a cyclic group is cyclic.');
  await page.waitForSelector('#tabs .tab:nth-child(3).dirty');
  await page.keyboard.press('Control+s');
  await page.waitForSelector('#tabs .tab:nth-child(3):not(.dirty)');
  if (!(await page.evaluate(() => window.__mock.disk['/notes/groups.fn'].includes('cyclic')))) fail('save did not write the active tab');

  const tabBox = async (n) => page.locator(`#tabs .tab:nth-child(${n})`).boundingBox();
  const from = await tabBox(3);
  const to = await tabBox(4);
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + from.width / 2 + 20, from.y + from.height / 2, { steps: 3 });
  await page.mouse.move(to.x + to.width * 0.8, to.y + to.height / 2, { steps: 8 });
  await page.mouse.up();
  if ((await names(page)).join() !== 'sample.fn,untitled.fn,limits.fn,groups.fn') fail(`drag did not reorder: ${await names(page)}`);

  if (shots) {
    await page.click('#tabs .tab:nth-child(4)');
    await page.waitForTimeout(400);
    await page.screenshot({ path: `${shots}/desktop-tabs.png` });
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.click('#dark');
    await page.waitForTimeout(400);
    await page.screenshot({ path: `${shots}/desktop-tabs-dark.png` });
    await page.click('#dark');
    await page.emulateMedia({ colorScheme: 'light' });
  }
  await page.click('#tabs .tab:nth-child(2)');
  await page.click('.cm-content');
  await page.keyboard.press('Control+w');
  await page.waitForTimeout(100);
  if ((await names(page)).length !== 4) fail('a dirty tab closed when the person said Cancel');
  await page.evaluate(() => { window.__mock.discard = true; });

  await page.click('#tabs .tab:nth-child(4)');
  await page.reload();
  await page.waitForSelector('.status.ok', { timeout: 10000 });
  if ((await names(page)).join() !== 'sample.fn,untitled.fn,limits.fn,groups.fn') fail(`restore order: ${await names(page)}`);
  if ((await activeName(page)) !== 'groups.fn') fail('restore did not keep the active tab');
  await page.click('#tabs .tab:nth-child(2)');
  if (!(await editorText(page)).includes('A draft line')) fail('restore lost the unsaved draft');
  if (!(await page.locator('#tabs .tab:nth-child(2).dirty').count())) fail('restored draft is not marked unsaved');

  await page.evaluate(() => { window.__mock.discard = true; });
  await page.click('.cm-content');
  await page.keyboard.press('Control+w');
  await page.waitForFunction(() => document.querySelectorAll('#tabs .tab').length === 3);
  for (let i = 0; i < 3; i++) await page.keyboard.press('Control+w');
  await page.waitForFunction(() => document.querySelectorAll('#tabs .tab').length === 1);
  if ((await names(page)).join() !== 'untitled.fn') fail(`closing all tabs leaves: ${await names(page)}`);

  if (errors.length) fail(`page errors: ${errors.join('; ')}`);
  if (!process.exitCode) console.log('desktop smoke test passed');
} finally {
  await browser.close();
  server.close();
}
