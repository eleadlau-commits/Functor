import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { forceLinting } from '@codemirror/lint';
import { loadFunctor, loadNative, isDesktop } from './functor.js';
import { functorLanguage, functorHighlight, functorCompletion, functorLinter } from './lang.js';
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

let fn;
let last = null;
let timer = 0;
let view;
let seq = 0;
let current = null;
let savedText = null;

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

async function render(src) {
  const mine = ++seq;
  lastSrc = src;
  const out = await fn.compile(src, widthPt);
  if (mine !== seq) return;
  last = out;
  previewEl.innerHTML = last.html;
  rebreak();
  const errors = last.diagnostics.filter((d) => d.severity === 'error').length;
  const warnings = last.diagnostics.length - errors;
  const parts = [fn.native ? `Compiled in ${last.nativeMs.toFixed(1)} ms (native)` : `Compiled in ${last.ms.toFixed(1)} ms`];
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
    forceLinting(view);
    if (view.hasFocus) view.dispatch({ effects: EditorView.scrollIntoView(view.state.selection.main.head, { y: 'nearest' }) });
  }
}

function schedule(src) {
  clearTimeout(timer);
  timer = setTimeout(() => {
    render(src);
    if (!fn.native) store.set(KEY_SRC, src);
    markDirty();
  }, fn.native ? 30 : 60);
}

function setSource(text) {
  const head = Math.min(view.state.selection.main.head, text.length);
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor: head } });
}

function markDirty() {
  if (!fn.native) return;
  const dirty = savedText !== null ? view.state.doc.toString() !== savedText : view.state.doc.length > 0;
  document.title = `${dirty ? '• ' : ''}${fileName()} · Functor`;
  fn.setTitle(document.title).catch(() => {});
}

function useDoc(doc) {
  current = doc.path;
  savedText = doc.text;
  nameEl.value = doc.name;
  if (view.state.doc.toString() !== doc.text) setSource(doc.text);
  markDirty();
}

function showError(err) {
  statusEl.textContent = String(err);
  statusEl.className = 'status bad';
}

async function saveNative(as) {
  const text = view.state.doc.toString();
  const doc = current && !as ? await fn.files.save(current, text) : await fn.files.saveAs(fileName(), text);
  if (doc) useDoc(doc);
}

function setupDesktop() {
  const files = fn.files;
  nameEl.readOnly = true;
  $('save').textContent = 'Save';
  $('save').title = 'Save the file (Ctrl+S). Shift-click to save as';
  $('open').title = 'Open a .fn file from disk';
  $('html').title = 'Export the compiled HTML';
  $('new').addEventListener('click', () => {
    current = null;
    savedText = null;
    files.close();
    markDirty();
  });
  $('open').addEventListener('click', () => files.open().then((d) => d && useDoc(d)).catch(showError));
  $('save').addEventListener('click', (e) => saveNative(e.shiftKey).catch(showError));
  $('html').addEventListener('click', () => standalone().then((html) => files.exportHtml(fileName().replace(/\.fn$/, '.html'), html)).catch(showError));
  $('pdf').title = 'Export a PDF set by Functor';
  $('pdf').addEventListener('click', () => files.exportPdf(fileName().replace(/\.fn$/, '.pdf'), view.state.doc.toString()).catch(showError));
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      saveNative(e.shiftKey).catch(showError);
    } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'o') {
      e.preventDefault();
      $('open').click();
    }
  });
  files.onChange((doc) => {
    const dirty = savedText !== null && view.state.doc.toString() !== savedText;
    if (doc.path !== current || dirty) return;
    useDoc(doc);
  });
}

function setupWeb() {
  nameEl.addEventListener('change', () => store.set(KEY_NAME, fileName()));
  $('open').addEventListener('click', () => $('file').click());
  $('file').addEventListener('change', async (e) => {
    const f = e.target.files[0];
    if (!f) return;
    nameEl.value = f.name;
    store.set(KEY_NAME, f.name);
    setSource(await f.text());
    e.target.value = '';
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
  $('new').addEventListener('click', () => store.set(KEY_NAME, nameEl.value));
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
  const info = await fn.symbols();
  const opened = fn.native ? await fn.files.initial() : null;
  const initial = opened ? opened.text : fn.native ? sample : store.get(KEY_SRC) ?? sample;
  nameEl.value = opened ? opened.name : fn.native ? 'sample.fn' : store.get(KEY_NAME) ?? (store.get(KEY_SRC) ? 'untitled.fn' : 'sample.fn');
  if (opened) {
    current = opened.path;
    savedText = opened.text;
  }

  view = new EditorView({
    parent: $('editor'),
    state: EditorState.create({
      doc: initial,
      extensions: [
        basicSetup,
        keymap.of([indentWithTab]),
        functorLanguage,
        functorHighlight,
        functorCompletion(fn, info),
        functorLinter(() => last),
        EditorView.lineWrapping,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) schedule(u.state.doc.toString());
        }),
      ],
    }),
  });
  await render(initial);
  markDirty();

  $('new').addEventListener('click', () => {
    nameEl.value = 'untitled.fn';
    setSource('---\nfunctor: 1\ntitle: Untitled\n---\n\n# Start\n\nWrite $x.hat$ here.\n');
  });
  if (fn.native) setupDesktop();
  else setupWeb();
  $('print').addEventListener('click', () => window.print());
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
