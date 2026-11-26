import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { forceLinting } from '@codemirror/lint';
import { loadFunctor, loadNative, isDesktop } from './functor.js';
import { functorLanguage, functorHighlight, functorCompletion, functorLinter } from './lang.js';
import { createInspector, traceAt } from './inspector.js';
import { patch } from './parts.js';
import { spellCheck } from './spell/index.js';
import { spellPanel } from './spell/panel.js';
import { createSync } from './sync.js';
import sample from '../../examples/sample.fn?raw';
import docCss from './doc.css?raw';
import refsJs from './refs.js?raw';
import './refs.js';
import './app.css';
import './doc.css';
import './fonts.css';
import lmRegular from './fonts/functor-lm-regular.woff2?url';
import lmItalic from './fonts/functor-lm-italic.woff2?url';
import lmBold from './fonts/functor-lm-bold.woff2?url';
import lmBoldItalic from './fonts/functor-lm-bolditalic.woff2?url';
import lmMono from './fonts/functor-lm-mono.woff2?url';

const KEY_SRC = 'functor.source';
const KEY_NAME = 'functor.name';
const KEY_DARK = 'functor.dark';
const KEY_TABS = 'functor.tabs';
const BLANK = '---\nfunctor: 1\ntitle: Untitled\n---\n\n# Start\n\nWrite $x.hat$ here.\n';

const store = {
  get(k) {
    try { return localStorage.getItem(k); } catch { return null; }
  },
  set(k, v) {
    try { localStorage.setItem(k, v); } catch {}
  },
};

const $ = (id) => document.getElementById(id);
const statusEl = $('status');
const listEl = $('list');
const previewEl = $('preview');
const nameEl = $('name');
const tabsEl = $('tabs');
const rightEl = document.querySelector('.right');

let fn;
let inspect = () => {};
let sync = null;
let last = null;
let timer = 0;
let view;
let wanted = null;
let running = null;
let extensions = [];
let tabs = [];
let active = null;
let nextId = 1;

let widthPt = 0;
let lastSrc = null;
let resizeTimer = 0;

function contentWidthPt() {
  const doc = previewEl.querySelector('.fn-doc');
  if (!doc) return 0;
  const cs = getComputedStyle(doc);
  const px = doc.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
  return Math.round(px * 0.75 * 100) / 100;
}

function rebreak() {
  const w = contentWidthPt();
  if (w > 0 && Math.abs(w - widthPt) > 0.5 && lastSrc !== null) {
    widthPt = w;
    render(lastSrc);
  }
}

new ResizeObserver(() => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(rebreak, 80);
}).observe(previewEl);

function render(src) {
  lastSrc = src;
  wanted = src;
  if (!running) running = pump();
  return running;
}

async function pump() {
  try {
    while (wanted !== null) {
      const src = wanted;
      wanted = null;
      const out = await fn.compile(src, widthPt);
      if (wanted === null) show(out);
    }
  } finally {
    running = null;
  }
}

function show(out) {
  last = out;
  if (active) active.out = out;
  patch(previewEl, last);
  if (window.functorNotes) window.functorNotes(previewEl);
  rebreak();
  const errors = last.diagnostics.filter((d) => d.severity === 'error').length;
  const warnings = last.diagnostics.length - errors;
  const parts = [fn.native ? `Compiled in ${last.nativeMs.toFixed(1)} ms (native)` : `Compiled in ${last.ms.toFixed(1)} ms`];
  parts.push(`${last.stats.rendered} of ${last.stats.rendered + last.stats.reused} blocks new`);
  parts.push(errors ? `${errors} error${errors > 1 ? 's' : ''}` : 'no errors');
  if (warnings) parts.push(`${warnings} warning${warnings > 1 ? 's' : ''}`);
  parts.push(`${last.stats.built} built, ${last.stats.hits} cached, ${last.stats.fromFont} from font`);
  statusEl.textContent = parts.join(' · ');
  statusEl.className = 'status ' + (errors ? 'bad' : warnings ? 'warn' : 'ok');
  listEl.replaceChildren(
    ...last.diagnostics.map((d) => {
      const li = document.createElement('li');
      li.className = d.severity;
      const loc = document.createElement('span');
      loc.className = 'loc';
      loc.textContent = `${d.line}:${d.col}`;
      const msg = document.createElement('span');
      msg.className = 'msg';
      msg.textContent = d.message;
      li.append(loc, msg);
      if (d.hint) {
        const hint = document.createElement('span');
        hint.className = 'hint';
        hint.textContent = d.hint;
        li.append(hint);
      }
      li.tabIndex = 0;
      const jump = () => {
        const doc = view.state.doc;
        const line = doc.line(Math.min(Math.max(d.line, 1), doc.lines));
        const pos = Math.min(line.from + d.col - 1, line.to);
        view.dispatch({ selection: { anchor: pos }, scrollIntoView: true });
        view.focus();
      };
      li.addEventListener('click', jump);
      li.addEventListener('keydown', (e) => { if (e.key === 'Enter') jump(); });
      return li;
    }),
  );
  if (view) {
    sync?.mark();
    inspectCursor(view.state);
    forceLinting(view);
    if (view.hasFocus) view.dispatch({ effects: EditorView.scrollIntoView(view.state.selection.main.head, { y: 'nearest' }) });
  }
}

function inspectCursor(state) {
  if (!last || !last.traces) return inspect(null);
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  inspect(traceAt(last.traces, line.number, head - line.from + 1));
}

function schedule(src) {
  clearTimeout(timer);
  timer = setTimeout(() => {
    render(src);
    markDirty();
    saveSession();
  }, fn.native || fn.worker ? 30 : 60);
}

function setSource(text) {
  const head = Math.min(view.state.selection.main.head, text.length);
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor: head } });
}

function textOf(tab) {
  return tab === active ? view.state.doc.toString() : tab.state.doc.toString();
}

function isDirty(tab) {
  return fn.native && textOf(tab) !== tab.saved;
}

function markDirty() {
  for (const el of tabsEl.querySelectorAll('.tab')) {
    const tab = tabs.find((t) => t.id === Number(el.dataset.id));
    if (tab) el.classList.toggle('dirty', isDirty(tab));
  }
  if (!fn.native) return;
  document.title = `${isDirty(active) ? '• ' : ''}${fileName()} · Functor`;
  fn.setTitle(document.title).catch(() => {});
}

function makeTab({ path = null, name, text, saved = text }) {
  return { id: nextId++, path, name, saved, state: EditorState.create({ doc: text, extensions }), scroll: 0, previewScroll: 0, out: null };
}

function freeName() {
  const used = new Set(tabs.map((t) => t.name));
  for (let i = 1; ; i++) {
    const n = i === 1 ? 'untitled.fn' : `untitled-${i}.fn`;
    if (!used.has(n)) return n;
  }
}

function stash() {
  if (!active) return;
  active.state = view.state;
  active.scroll = view.scrollDOM.scrollTop;
  active.previewScroll = rightEl.scrollTop;
}

function activate(tab) {
  if (tab === active) return;
  stash();
  active = tab;
  view.setState(tab.state);
  nameEl.value = tab.name;
  last = tab.out;
  if (last) patch(previewEl, last);
  requestAnimationFrame(() => {
    view.scrollDOM.scrollTop = tab.scroll;
    rightEl.scrollTop = tab.previewScroll;
  });
  renderTabs();
  markDirty();
  saveSession();
  render(view.state.doc.toString()).then(() => { rightEl.scrollTop = tab.previewScroll; });
  view.focus();
}

function addTab(tab, at = tabs.indexOf(active) + 1) {
  tabs.splice(at > 0 ? at : tabs.length, 0, tab);
  activate(tab);
}

function newTab() {
  addTab(makeTab({ name: freeName(), text: BLANK }));
}

async function closeTab(tab) {
  if (isDirty(tab) && !(await fn.files.askDiscard(tab.name))) return;
  const i = tabs.indexOf(tab);
  tabs.splice(i, 1);
  if (fn.native && tab.path && !tabs.some((t) => t.path === tab.path)) fn.files.close(tab.path).catch(() => {});
  if (!tabs.length) tabs.push(makeTab({ name: freeName(), text: BLANK }));
  if (tab === active) {
    active = null;
    activate(tabs[Math.min(i, tabs.length - 1)]);
  } else {
    renderTabs();
    saveSession();
  }
}

function cycle(step) {
  const i = tabs.indexOf(active);
  activate(tabs[(i + step + tabs.length) % tabs.length]);
}

function renderTabs() {
  tabsEl.replaceChildren(
    ...tabs.map((tab) => {
      const el = document.createElement('div');
      el.className = 'tab';
      el.dataset.id = tab.id;
      el.setAttribute('role', 'tab');
      el.setAttribute('aria-selected', String(tab === active));
      el.tabIndex = tab === active ? 0 : -1;
      el.title = tab.path || tab.name;
      el.classList.toggle('dirty', isDirty(tab));
      const label = document.createElement('span');
      label.className = 'tab-name';
      label.textContent = tab.name;
      const close = document.createElement('button');
      close.className = 'tab-close';
      close.title = 'Close tab';
      close.setAttribute('aria-label', `Close ${tab.name}`);
      close.tabIndex = -1;
      close.textContent = '×';
      close.addEventListener('pointerdown', (e) => e.stopPropagation());
      close.addEventListener('click', (e) => {
        e.stopPropagation();
        closeTab(tab);
      });
      el.append(label, close);
      el.addEventListener('auxclick', (e) => {
        if (e.button === 1) closeTab(tab);
      });
      el.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
          cycle(e.key === 'ArrowRight' ? 1 : -1);
          tabsEl.querySelector('[aria-selected="true"]')?.focus();
        } else if (e.key === 'Delete') closeTab(tab);
      });
      el.addEventListener('pointerdown', (e) => dragTab(e, el, tab));
      return el;
    }),
  );
  const add = document.createElement('button');
  add.className = 'tab-add';
  add.title = fn.native ? 'New tab (Ctrl+T)' : 'New tab';
  add.setAttribute('aria-label', 'New tab');
  add.textContent = '+';
  add.addEventListener('click', newTab);
  tabsEl.append(add);
  tabsEl.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

function dragTab(e, el, tab) {
  if (e.button !== 0) return;
  const x0 = e.clientX;
  let moved = false;
  el.setPointerCapture(e.pointerId);
  const move = (ev) => {
    if (!moved && Math.abs(ev.clientX - x0) < 5) return;
    moved = true;
    el.classList.add('dragging');
    for (const other of tabsEl.querySelectorAll('.tab')) {
      if (other === el) continue;
      const r = other.getBoundingClientRect();
      const mid = r.left + r.width / 2;
      const before = other.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING;
      if (before && ev.clientX < mid && ev.clientX > r.left) other.before(el);
      else if (!before && ev.clientX > mid && ev.clientX < r.right) other.after(el);
    }
  };
  const up = () => {
    el.removeEventListener('pointermove', move);
    el.removeEventListener('pointerup', up);
    el.removeEventListener('pointercancel', up);
    el.classList.remove('dragging');
    if (moved) {
      const order = [...tabsEl.querySelectorAll('.tab')].map((n) => Number(n.dataset.id));
      tabs.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
      saveSession();
    } else activate(tab);
  };
  el.addEventListener('pointermove', move);
  el.addEventListener('pointerup', up);
  el.addEventListener('pointercancel', up);
}

function saveSession() {
  if (!active) return;
  const list = tabs.map((t) => {
    const text = textOf(t);
    if (!fn.native) return { name: t.name, text };
    const entry = { path: t.path, name: t.name };
    if (!t.path || text !== t.saved) entry.text = text;
    if (!t.path) entry.saved = t.saved;
    return entry;
  });
  store.set(KEY_TABS, JSON.stringify({ tabs: list, active: tabs.indexOf(active) }));
}

function readSession() {
  try {
    const s = JSON.parse(store.get(KEY_TABS));
    return s && Array.isArray(s.tabs) && s.tabs.length ? s : null;
  } catch {
    return null;
  }
}

async function restoreNative(session) {
  const out = [];
  for (const e of session?.tabs ?? []) {
    if (e.path) {
      const doc = await fn.files.load(e.path).catch(() => null);
      if (doc) out.push(makeTab({ path: doc.path, name: doc.name, text: e.text ?? doc.text, saved: doc.text }));
      else if (e.text != null) out.push(makeTab({ name: e.name, text: e.text, saved: '' }));
    } else if (typeof e.text === 'string') out.push(makeTab({ name: e.name || 'untitled.fn', text: e.text, saved: e.saved ?? '' }));
  }
  return out;
}

function restoreWeb(session) {
  if (session) return session.tabs.filter((e) => typeof e.text === 'string').map((e) => makeTab({ name: e.name || 'untitled.fn', text: e.text }));
  const src = store.get(KEY_SRC);
  if (src === null) return [];
  return [makeTab({ name: store.get(KEY_NAME) ?? 'untitled.fn', text: src })];
}

function openDoc(doc) {
  const have = tabs.find((t) => t.path === doc.path);
  if (have) return activate(have);
  const tab = makeTab({ path: doc.path, name: doc.name, text: doc.text });
  const blank = !active.path && textOf(active) === active.saved && tabs.length === 1;
  if (blank) {
    tabs = [tab];
    active = null;
    activate(tab);
  } else addTab(tab);
}

function useDoc(tab, doc) {
  const moved = tab.path && tab.path !== doc.path;
  if (moved && !tabs.some((t) => t !== tab && t.path === tab.path)) fn.files.close(tab.path).catch(() => {});
  tab.path = doc.path;
  tab.name = doc.name;
  tab.saved = doc.text;
  if (textOf(tab) !== doc.text) {
    if (tab === active) setSource(doc.text);
    else {
      tab.state = tab.state.update({ changes: { from: 0, to: tab.state.doc.length, insert: doc.text } }).state;
      tab.out = null;
    }
  }
  if (tab === active) nameEl.value = doc.name;
  renderTabs();
  markDirty();
  saveSession();
}

function showError(err) {
  statusEl.textContent = String(err);
  statusEl.className = 'status bad';
}

async function saveNative(as) {
  const tab = active;
  const text = view.state.doc.toString();
  const doc = tab.path && !as ? await fn.files.save(tab.path, text) : await fn.files.saveAs(fileName(), text);
  if (doc) useDoc(tab, doc);
}

function setupDesktop() {
  const files = fn.files;
  nameEl.hidden = true;
  $('save').textContent = 'Save';
  $('save').title = 'Save the file (Ctrl+S). Shift-click to save as';
  $('open').title = 'Open a .fn file from disk';
  $('html').title = 'Export the compiled HTML';
  $('new').title = 'New tab (Ctrl+T)';
  $('open').title = 'Open .fn files from disk in new tabs (Ctrl+O)';
  $('open').addEventListener('click', () => files.open().then((docs) => docs.forEach(openDoc)).catch(showError));
  $('save').addEventListener('click', (e) => saveNative(e.shiftKey).catch(showError));
  $('html').addEventListener('click', () => standalone().then((html) => files.exportHtml(fileName().replace(/\.fn$/, '.html'), html)).catch(showError));
  $('pdf').title = 'Export a PDF set by Functor';
  $('pdf').addEventListener('click', () => files.exportPdf(fileName().replace(/\.fn$/, '.pdf'), view.state.doc.toString()).catch(showError));
  document.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    const k = e.key.toLowerCase();
    let act = null;
    if (mod && k === 's') act = () => saveNative(e.shiftKey).catch(showError);
    else if (mod && k === 'o') act = () => $('open').click();
    else if (mod && k === 't') act = newTab;
    else if (mod && k === 'w') act = () => closeTab(active);
    else if (e.ctrlKey && k === 'tab') act = () => cycle(e.shiftKey ? -1 : 1);
    else if (mod && (k === 'pagedown' || k === 'pageup')) act = () => cycle(k === 'pagedown' ? 1 : -1);
    else if (mod && !e.shiftKey && !e.altKey && /^[1-9]$/.test(k)) act = () => activate(k === '9' ? tabs[tabs.length - 1] : tabs[Number(k) - 1] ?? active);
    if (!act) return;
    e.preventDefault();
    act();
  });
  files.onMenu((id) => {
    if (id === 'new-tab') newTab();
    else if (id === 'close-tab') closeTab(active);
    else if (id === 'open') $('open').click();
    else if (id === 'next-tab') cycle(1);
    else if (id === 'prev-tab') cycle(-1);
  });
  files.onChange((doc) => {
    for (const tab of tabs.filter((t) => t.path === doc.path)) {
      if (!isDirty(tab)) useDoc(tab, doc);
    }
  });
  window.addEventListener('pagehide', saveSession);
}

function setupWeb() {
  nameEl.addEventListener('change', () => {
    active.name = fileName();
    nameEl.value = active.name;
    renderTabs();
    saveSession();
  });
  $('open').addEventListener('click', () => $('file').click());
  $('file').addEventListener('change', async (e) => {
    const f = e.target.files[0];
    if (!f) return;
    const text = await f.text();
    e.target.value = '';
    const blank = textOf(active) === BLANK && tabs.length === 1;
    const tab = makeTab({ name: f.name, text });
    if (blank) {
      tabs = [tab];
      active = null;
      activate(tab);
    } else addTab(tab);
  });
  $('save').addEventListener('click', () => download(fileName(), view.state.doc.toString(), 'text/plain'));
  $('html').addEventListener('click', () => standalone().then((html) => download(fileName().replace(/\.fn$/, '.html'), html, 'text/html')).catch(showError));
  $('pdf').addEventListener('click', () => {
    try {
      download(fileName().replace(/\.fn$/, '.pdf'), fn.pdf(view.state.doc.toString()), 'application/pdf');
    } catch (e) {
      showError(e);
    }
  });
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      download(fileName(), view.state.doc.toString(), 'text/plain');
    }
  });
}

function download(name, text, type) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function fileName() {
  const n = nameEl.value.trim() || 'untitled.fn';
  return n.endsWith('.fn') ? n : n + '.fn';
}

async function dataUrl(url) {
  const blob = await (await fetch(url)).blob();
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result);
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

async function fontFaces() {
  const faces = [['Functor LM', 'normal', 400, lmRegular], ['Functor LM', 'italic', 400, lmItalic], ['Functor LM', 'normal', 700, lmBold], ['Functor LM', 'italic', 700, lmBoldItalic], ['Functor LM Mono', 'normal', 400, lmMono]];
  const css = await Promise.all(faces.map(async ([family, style, weight, url]) => `@font-face{font-family:"${family}";font-style:${style};font-weight:${weight};font-display:block;src:url(${await dataUrl(url)}) format("woff2")}`));
  return css.join('');
}

async function standalone() {
  const fonts = await fontFaces().catch(() => '');
  const title = fileName().replace(/\.fn$/, '');
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title.replace(/[<&]/g, '')}</title><style>${fonts}${docCss}</style></head><body>${last ? last.html : ''}<script>${refsJs}functorRefs(document.body);</script></body></html>`;
}

function setupDivider() {
  const div = $('divider');
  const panes = document.querySelector('.panes');
  div.addEventListener('pointerdown', (e) => {
    div.setPointerCapture(e.pointerId);
    const move = (ev) => {
      const r = panes.getBoundingClientRect();
      const pct = Math.min(80, Math.max(20, ((ev.clientX - r.left) / r.width) * 100));
      panes.style.setProperty('--split', pct + '%');
    };
    const up = () => {
      div.removeEventListener('pointermove', move);
      div.removeEventListener('pointerup', up);
    };
    div.addEventListener('pointermove', move);
    div.addEventListener('pointerup', up);
  });
}

async function main() {
  fn = isDesktop ? await loadNative() : await loadFunctor(new URL('functor.wasm', document.baseURI).href);
  inspect = createInspector($('inspector'), fn);
  const info = await fn.symbols();
  extensions = [
    basicSetup,
    keymap.of([indentWithTab, { key: 'Alt-Enter', run: () => sync?.toPreview() ?? false }]),
    functorLanguage,
    functorHighlight,
    functorCompletion(fn, info),
    functorLinter(() => last),
    spellCheck(),
    EditorView.lineWrapping,
    EditorView.updateListener.of((u) => {
      if (u.docChanged) schedule(u.state.doc.toString());
      else if (u.selectionSet) {
        inspectCursor(u.state);
        sync?.mark();
      }
    }),
  ];
  const session = readSession();
  tabs = fn.native ? await restoreNative(session) : restoreWeb(session);
  if (!tabs.length) tabs.push(makeTab({ name: 'sample.fn', text: sample }));
  const first = tabs[Math.min(Math.max(session?.active ?? 0, 0), tabs.length - 1)];
  view = new EditorView({ parent: $('editor'), state: first.state });
  sync = createSync(view, previewEl);
  $('locate').addEventListener('click', () => sync.toPreview());
  activate(first);
  if (fn.native) {
    const opened = await fn.files.initial(!session).catch(() => null);
    if (opened) openDoc(opened);
  }
  $('new').addEventListener('click', newTab);
  if (fn.native) setupDesktop();
  else setupWeb();
  $('print').addEventListener('click', () => window.print());
  spellPanel($('spell'));
  window.functorRefs(previewEl);
  const dark = $('dark');
  const setDark = (on) => {
    previewEl.classList.toggle('fn-dark', on);
    dark.setAttribute('aria-pressed', String(on));
    store.set(KEY_DARK, on ? '1' : '0');
  };
  setDark(store.get(KEY_DARK) === '1');
  dark.addEventListener('click', () => setDark(!previewEl.classList.contains('fn-dark')));
  $('toggle').addEventListener('click', (e) => {
    const open = document.body.classList.toggle('problems-closed');
    e.target.setAttribute('aria-expanded', String(!open));
  });
  setupDivider();
}

main().catch((err) => {
  statusEl.textContent = `The compiler did not load: ${err.message}`;
  statusEl.className = 'status bad';
});
