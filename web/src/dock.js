import { PANELS, setBadge } from './panels.js';
import { openLink, native, iconButton, ICONS } from './link.js';

const KEY = 'functor.dock';
const PLACES = ['bottom', 'side'];
const BLOCKED = 'The browser blocked the new window. Allow pop-ups for this page, then try again.';

function readSaved(store) {
  try {
    return JSON.parse(store.get(KEY)) || {};
  } catch {
    return {};
  }
}

export async function createDock({ store, docks, toggles, onJump, onNote }) {
  const id = native ? 'desktop' : Math.random().toString(36).slice(2, 10);
  const names = Object.keys(PANELS);
  const saved = readSaved(store);
  const state = {};
  const data = {};
  const els = {};
  const wins = {};

  for (const name of names) {
    const s = saved[name] || {};
    const home = PLACES.includes(s.home) ? s.home : 'bottom';
    state[name] = { home, place: s.place === 'hidden' ? 'hidden' : home };
    data[name] = PANELS[name].empty;
    els[name] = build(name);
    toggles[name].addEventListener('click', () => toggle(name));
  }

  const send = await openLink(id, onMessage);
  layout();
  names.forEach(paint);

  window.addEventListener('pagehide', () => {
    if (!native) send({ from: 'main', type: 'close' });
  });

  function build(name) {
    const p = PANELS[name];
    const el = document.createElement('section');
    el.className = 'dock-panel';
    el.id = `panel-${name}`;
    el.dataset.panel = name;
    el.setAttribute('aria-label', p.label);
    const head = document.createElement('header');
    head.className = 'dock-head';
    const title = document.createElement('span');
    title.className = 'dock-title';
    title.textContent = p.title;
    const badge = document.createElement('span');
    badge.className = 'dock-badge';
    badge.hidden = true;
    const tools = document.createElement('span');
    tools.className = 'dock-tools';
    const move = iconButton('side', 'Move to the side', 'move');
    const pop = iconButton('pop', 'Open in its own window', 'pop');
    const hide = iconButton('close', 'Hide the panel', 'hide');
    move.addEventListener('click', () => moveTo(name));
    pop.addEventListener('click', () => popOut(name));
    hide.addEventListener('click', () => setPlace(name, 'hidden'));
    tools.append(move, pop, hide);
    head.append(title, badge, tools);
    head.addEventListener('dblclick', (e) => {
      if (!e.target.closest('button')) popOut(name);
    });
    const body = document.createElement('div');
    body.className = 'dock-body';
    if (name === 'symbol') body.setAttribute('aria-live', 'polite');
    el.append(head, body);
    return el;
  }

  function docked(name) {
    return PLACES.includes(state[name].place);
  }

  function layout() {
    for (const name of names) {
      const { place } = state[name];
      const el = els[name];
      if (PLACES.includes(place)) {
        docks[place].append(el);
        el.hidden = false;
        const other = place === 'bottom' ? 'side' : 'bottom';
        const move = el.querySelector('[data-act="move"]');
        move.innerHTML = ICONS[other];
        move.title = other === 'side' ? 'Move to the side' : 'Move to the bottom';
        move.setAttribute('aria-label', move.title);
      } else el.hidden = true;
      const t = toggles[name];
      t.setAttribute('aria-pressed', String(place !== 'hidden'));
      t.title = place === 'window' ? `${PANELS[name].title} is in its own window. Click to show it` : place === 'hidden' ? `Show the ${PANELS[name].title} panel` : `Hide the ${PANELS[name].title} panel`;
    }
    for (const place of PLACES) {
      const any = [...docks[place].children].some((c) => !c.hidden);
      docks[place].hidden = !any;
      document.body.classList.toggle(`has-${place}-dock`, any);
    }
    store.set(KEY, JSON.stringify(state));
  }

  function paint(name) {
    const p = PANELS[name];
    if (docked(name)) {
      const el = els[name];
      p.render(el.querySelector('.dock-body'), data[name], onJump);
      setBadge(el.querySelector('.dock-badge'), p.badge(data[name]));
    } else if (state[name].place === 'window') {
      send({ from: 'main', type: 'state', panel: name, data: data[name] });
    }
  }

  function setPlace(name, place) {
    state[name].place = place;
    if (PLACES.includes(place)) state[name].home = place;
    layout();
    paint(name);
  }

  function moveTo(name) {
    setPlace(name, state[name].place === 'side' ? 'bottom' : 'side');
  }

  function toggle(name) {
    const { place, home } = state[name];
    if (place === 'window') focusWin(name);
    else setPlace(name, place === 'hidden' ? home : 'hidden');
  }

  function back(name) {
    if (state[name].place !== 'window') return;
    wins[name] = null;
    setPlace(name, state[name].home);
  }

  function panelUrl(name) {
    return `panel.html?p=${name}&id=${id}`;
  }

  async function popOut(name) {
    if (state[name].place === 'window') return focusWin(name);
    const ok = native ? await popNative(name) : popWeb(name);
    if (!ok) return;
    state[name].place = 'window';
    layout();
  }

  function popWeb(name) {
    const w = window.open(panelUrl(name), `functor-${id}-${name}`, 'popup,width=560,height=440');
    if (!w) {
      onNote(BLOCKED);
      return false;
    }
    wins[name] = w;
    const timer = setInterval(() => {
      if (!w.closed) return;
      clearInterval(timer);
      if (wins[name] === w) back(name);
    }, 500);
    return true;
  }

  async function popNative(name) {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const label = `panel-${name}`;
    const have = await WebviewWindow.getByLabel(label).catch(() => null);
    if (have) await have.close().catch(() => {});
    const w = new WebviewWindow(label, {
      url: panelUrl(name),
      title: `${PANELS[name].title} · Functor`,
      width: 560,
      height: 440,
      minWidth: 300,
      minHeight: 180,
    });
    return new Promise((resolve) => {
      w.once('tauri://created', () => {
        wins[name] = w;
        resolve(true);
      });
      w.once('tauri://error', (e) => {
        onNote(`The window did not open: ${e.payload ?? 'unknown error'}`);
        resolve(false);
      });
      w.once('tauri://destroyed', () => {
        if (wins[name] === w) back(name);
      });
    });
  }

  function focusWin(name) {
    const w = wins[name];
    if (!w) return back(name);
    if (native) w.setFocus().catch(() => {});
    else w.focus();
  }

  function closeWin(name) {
    const w = wins[name];
    wins[name] = null;
    if (w) Promise.resolve(w.close()).catch(() => {});
  }

  async function focusMain() {
    if (!native) return window.focus();
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    getCurrentWindow().setFocus().catch(() => {});
  }

  function onMessage(msg) {
    const name = msg.from;
    if (!PANELS[name]) return;
    if (msg.type === 'hello') {
      send({ from: 'main', type: 'state', panel: name, data: data[name] });
    } else if (msg.type === 'jump') {
      onJump(msg.line, msg.col);
      focusMain();
    } else if (msg.type === 'dock') {
      const w = wins[name];
      back(name);
      wins[name] = w;
      closeWin(name);
    }
  }

  return {
    update(name, value) {
      data[name] = value;
      paint(name);
    },
    state: () => JSON.parse(JSON.stringify(state)),
  };
}
