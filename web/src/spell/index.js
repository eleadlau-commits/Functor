import { ViewPlugin, Decoration, EditorView, keymap } from '@codemirror/view';
import { proseWords } from './scan.js';
import affGb from '../../node_modules/dictionary-en-gb/index.aff?url';
import dicGb from '../../node_modules/dictionary-en-gb/index.dic?url';
import affUs from '../../node_modules/dictionary-en/index.aff?url';
import dicUs from '../../node_modules/dictionary-en/index.dic?url';

export { proseWords };

const KEY_ON = 'functor.spell';
const KEY_LANG = 'functor.spell.lang';
const KEY_WORDS = 'functor.words';

export const LANGS = {
  'en-GB': { label: 'English (UK)', aff: affGb, dic: dicGb },
  'en-US': { label: 'English (US)', aff: affUs, dic: dicUs },
};

const store = {
  get(k) {
    try { return localStorage.getItem(k); } catch { return null; }
  },
  set(k, v) {
    try { localStorage.setItem(k, v); } catch {}
  },
};

const plain = (w) => w.replace(/’/g, "'");

function readWords() {
  try {
    const list = JSON.parse(store.get(KEY_WORDS));
    return Array.isArray(list) ? list.filter((w) => typeof w === 'string') : [];
  } catch {
    return [];
  }
}

class Speller {
  constructor() {
    this.on = store.get(KEY_ON) !== '0';
    this.lang = LANGS[store.get(KEY_LANG)] ? store.get(KEY_LANG) : 'en-GB';
    this.personal = new Set(readWords());
    this.cache = new Map();
    this.views = new Set();
    this.worker = null;
    this.calls = new Map();
    this.next = 1;
    this.listeners = new Set();
    this.error = null;
  }

  call(msg) {
    if (!this.worker) this.start();
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.calls.set(id, { resolve, reject });
      this.worker.postMessage({ ...msg, id });
    });
  }

  start() {
    this.worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
    this.worker.onmessage = ({ data }) => {
      const c = this.calls.get(data.id);
      if (!c) return;
      this.calls.delete(data.id);
      if (data.error) c.reject(new Error(data.error));
      else c.resolve(data);
    };
    this.worker.onerror = (e) => {
      this.error = e.message || 'The spell checker did not start.';
      for (const c of this.calls.values()) c.reject(new Error(this.error));
      this.calls.clear();
      this.emit();
    };
    this.loading = this.call({ type: 'load', dict: LANGS[this.lang], personal: [...this.personal] });
    this.loading.then(() => { this.error = null; this.emit(); }, (e) => { this.error = e.message; this.emit(); });
  }

  async check(words) {
    const todo = [...new Set(words.map(plain))].filter((w) => !this.cache.has(w));
    if (todo.length) {
      const lang = this.lang;
      const { bad } = await this.call({ type: 'check', words: todo });
      if (lang !== this.lang) return;
      const set = new Set(bad);
      for (const w of todo) this.cache.set(w, set.has(w));
    }
  }

  wrong(word) {
    const w = plain(word);
    return this.cache.get(w) === true && !this.personal.has(w);
  }

  suggest(word) {
    return this.call({ type: 'suggest', word }).then((r) => r.list);
  }

  add(word) {
    const w = plain(word);
    this.personal.add(w);
    store.set(KEY_WORDS, JSON.stringify([...this.personal].sort()));
    if (this.worker) this.call({ type: 'add', word: w }).catch(() => {});
    this.refresh();
  }

  remove(word) {
    const w = plain(word);
    if (!this.personal.delete(w)) return;
    store.set(KEY_WORDS, JSON.stringify([...this.personal].sort()));
    this.reload();
  }

  setOn(on) {
    this.on = on;
    store.set(KEY_ON, on ? '1' : '0');
    this.refresh();
  }

  setLang(lang) {
    if (!LANGS[lang] || lang === this.lang) return;
    this.lang = lang;
    store.set(KEY_LANG, lang);
    this.reload();
  }

  reload() {
    this.cache.clear();
    if (this.worker) {
      this.loading = this.call({ type: 'load', dict: LANGS[this.lang], personal: [...this.personal] });
      this.loading.catch((e) => { this.error = e.message; this.emit(); });
    }
    this.refresh();
  }

  refresh() {
    for (const v of this.views) v.run(0);
    this.emit();
  }

  emit() {
    for (const f of this.listeners) f(this);
  }

  onChange(f) {
    this.listeners.add(f);
    f(this);
    return () => this.listeners.delete(f);
  }
}

export const speller = new Speller();

const mark = Decoration.mark({ class: 'cm-misspelled' });

const plugin = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.view = view;
      this.deco = Decoration.none;
      this.words = [];
      this.timer = 0;
      speller.views.add(this);
      this.run(0);
    }

    update(u) {
      if (u.docChanged) {
        this.deco = this.deco.map(u.changes);
        this.run(400);
      }
    }

    run(delay) {
      clearTimeout(this.timer);
      this.timer = setTimeout(() => this.scan(), delay);
    }

    async scan() {
      if (!speller.on) {
        this.set(Decoration.none);
        return;
      }
      const doc = this.view.state.doc;
      const words = proseWords(doc.toString());
      try {
        await speller.check(words.map((w) => w.word));
      } catch {
        return;
      }
      if (this.view.state.doc !== doc) return;
      this.words = words;
      this.set(Decoration.set(words.filter((w) => speller.wrong(w.word)).map((w) => mark.range(w.from, w.to))));
    }

    set(deco) {
      this.deco = deco;
      this.view.dispatch({});
    }

    at(pos) {
      let hit = null;
      this.deco.between(pos, pos, (from, to) => {
        hit = { from, to };
        return false;
      });
      return hit;
    }

    destroy() {
      clearTimeout(this.timer);
      speller.views.delete(this);
      closeMenu();
    }
  },
  { decorations: (p) => p.deco },
);

let menu = null;

function closeMenu(focus) {
  if (!menu) return;
  const { el, view } = menu;
  menu = null;
  el.remove();
  document.removeEventListener('pointerdown', outside, true);
  if (focus) view.focus();
}

function outside(e) {
  if (menu && !menu.el.contains(e.target)) closeMenu(false);
}

function matchCase(orig, s) {
  if (orig[0] && orig[0] === orig[0].toUpperCase() && s[0] === s[0].toLowerCase()) return s[0].toUpperCase() + s.slice(1);
  return s;
}

function item(label, act, cls) {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = 'cm-spell-item' + (cls ? ' ' + cls : '');
  b.setAttribute('role', 'menuitem');
  b.tabIndex = -1;
  b.textContent = label;
  b.addEventListener('click', act);
  return b;
}

function openMenu(view, range, x, y) {
  closeMenu(false);
  const word = view.state.sliceDoc(range.from, range.to);
  const el = document.createElement('div');
  el.className = 'cm-spell-menu';
  el.setAttribute('role', 'menu');
  el.setAttribute('aria-label', `Spelling of ${word}`);
  const head = document.createElement('div');
  head.className = 'cm-spell-head';
  head.textContent = `“${word}” is not in the ${LANGS[speller.lang].label} dictionary`;
  const list = document.createElement('div');
  list.className = 'cm-spell-list';
  const wait = document.createElement('div');
  wait.className = 'cm-spell-note';
  wait.textContent = 'Finding suggestions…';
  list.append(wait);
  const replace = (s) => () => {
    if (view.state.sliceDoc(range.from, range.to) === word) {
      view.dispatch({ changes: { from: range.from, to: range.to, insert: s }, selection: { anchor: range.from + s.length }, userEvent: 'input.spell' });
    }
    closeMenu(true);
  };
  const add = item('Add to my word list', () => {
    speller.add(word);
    closeMenu(true);
  }, 'cm-spell-add');
  el.append(head, list, add);
  el.addEventListener('keydown', (e) => {
    const items = [...el.querySelectorAll('.cm-spell-item')];
    const i = items.indexOf(document.activeElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = e.key === 'ArrowDown' ? i + 1 : i - 1;
      items[(n + items.length) % items.length]?.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeMenu(true);
    } else if (e.key === 'Tab') {
      e.preventDefault();
    }
  });
  document.body.append(el);
  const r = el.getBoundingClientRect();
  el.style.left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4)) + 'px';
  el.style.top = (y + r.height > window.innerHeight - 4 ? Math.max(4, y - r.height - 24) : y) + 'px';
  menu = { el, view };
  document.addEventListener('pointerdown', outside, true);
  add.focus();
  speller.suggest(word).then(
    (found) => {
      if (!menu || menu.el !== el) return;
      const opts = [...new Set(found.map((s) => matchCase(word, s)))].filter((s) => s !== word);
      if (!opts.length) wait.textContent = 'No suggestions';
      else {
        list.replaceChildren(...opts.map((s) => item(s, replace(s), 'cm-spell-fix')));
        list.firstChild.focus();
      }
    },
    () => {
      if (menu && menu.el === el) wait.textContent = 'No suggestions';
    },
  );
}

function menuAt(view, pos, x, y) {
  const p = view.plugin(plugin);
  const range = p && p.at(pos);
  if (!range) return false;
  if (x == null) {
    const c = view.coordsAtPos(range.from);
    if (!c) return false;
    x = c.left;
    y = c.bottom + 4;
  }
  openMenu(view, range, x, y);
  return true;
}

function nextWrong(view) {
  const p = view.plugin(plugin);
  if (!p) return false;
  const head = view.state.selection.main.head;
  let first = null;
  let after = null;
  p.deco.between(0, view.state.doc.length, (from, to) => {
    if (!first) first = { from, to };
    if (!after && from > head) after = { from, to };
  });
  const hit = after || first;
  if (!hit) return false;
  view.dispatch({ selection: { anchor: hit.from, head: hit.to }, scrollIntoView: true });
  return true;
}

const theme = EditorView.baseTheme({
  '.cm-misspelled': {
    textDecoration: 'underline wavy var(--spell, #d6336c)',
    textDecorationThickness: '1px',
    textUnderlineOffset: '3px',
    textDecorationSkipInk: 'none',
  },
});

export function spellCheck() {
  return [
    plugin,
    theme,
    EditorView.domEventHandlers({
      contextmenu(e, view) {
        if (!speller.on) return false;
        const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
        if (pos == null) return false;
        if (!menuAt(view, pos, e.clientX, e.clientY)) return false;
        e.preventDefault();
        return true;
      },
    }),
    keymap.of([
      { key: 'Mod-.', run: (view) => speller.on && menuAt(view, view.state.selection.main.head) },
      { key: 'Alt-F7', run: (view) => speller.on && nextWrong(view) },
    ]),
  ];
}
