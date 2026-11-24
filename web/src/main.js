import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { forceLinting } from '@codemirror/lint';
import { loadFunctor } from './functor.js';
import { functorLanguage, functorHighlight, functorCompletion, functorLinter } from './lang.js';
import sample from '../../examples/sample.fn?raw';
import docCss from './doc.css?raw';
import './app.css';
import './doc.css';

const KEY_SRC = 'functor.source';
const KEY_NAME = 'functor.name';

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

function render(src) {
  last = fn.compile(src);
  previewEl.innerHTML = last.html;
  const errors = last.diagnostics.filter((d) => d.severity === 'error').length;
  const warnings = last.diagnostics.length - errors;
  const parts = [`Compiled in ${last.ms.toFixed(1)} ms`];
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
    store.set(KEY_SRC, src);
  }, 60);
}

function setSource(text) {
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
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

function standalone() {
  const title = fileName().replace(/\.fn$/, '');
  return `<!doctype html><html><head><meta charset="utf-8"><title>${title.replace(/[<&]/g, '')}</title><style>${docCss}</style></head><body>${last ? last.html : ''}</body></html>`;
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
  fn = await loadFunctor(new URL('functor.wasm', document.baseURI).href);
  const info = fn.symbols();
  const initial = store.get(KEY_SRC) ?? sample;
  nameEl.value = store.get(KEY_NAME) ?? (store.get(KEY_SRC) ? 'untitled.fn' : 'sample.fn');

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
  render(initial);

  nameEl.addEventListener('change', () => store.set(KEY_NAME, fileName()));
  $('new').addEventListener('click', () => {
    nameEl.value = 'untitled.fn';
    store.set(KEY_NAME, nameEl.value);
    setSource('---\nfunctor: 1\ntitle: Untitled\n---\n\n# Start\n\nWrite $x.hat$ here.\n');
  });
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
  $('html').addEventListener('click', () => download(fileName().replace(/\.fn$/, '.html'), standalone(), 'text/html'));
  $('print').addEventListener('click', () => window.print());
  $('toggle').addEventListener('click', (e) => {
    const open = document.body.classList.toggle('problems-closed');
    e.target.setAttribute('aria-expanded', String(!open));
  });
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      download(fileName(), view.state.doc.toString(), 'text/plain');
    }
  });
  setupDivider();
}

main().catch((err) => {
  statusEl.textContent = `The compiler did not load: ${err.message}`;
  statusEl.className = 'status bad';
});
