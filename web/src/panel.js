import { PANELS, setBadge } from './panels.js';
import { openLink, native, iconButton } from './link.js';
import './app.css';

const q = new URLSearchParams(location.search);
const name = q.get('p');
const id = q.get('id') || '';
const p = PANELS[name];
const root = document.getElementById('panel');

async function closeSelf() {
  if (!native) return window.close();
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  getCurrentWindow().close().catch(() => {});
}

async function main() {
  if (!p) {
    root.textContent = 'This panel does not exist.';
    return;
  }
  document.title = `${p.title} · Functor`;
  root.dataset.panel = name;
  root.setAttribute('aria-label', p.label);
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
  const dock = iconButton('dock', 'Put the panel back in the editor window', 'dock');
  tools.append(dock);
  head.append(title, badge, tools);
  const body = document.createElement('div');
  body.className = 'dock-body';
  if (name === 'symbol') body.setAttribute('aria-live', 'polite');
  root.append(head, body);
  const show = (data) => {
    p.render(body, data, (line, col) => send({ from: name, type: 'jump', line, col }));
    setBadge(badge, p.badge(data));
  };
  show(p.empty);
  const send = await openLink(id, (msg) => {
    if (msg.from !== 'main') return;
    if (msg.type === 'state' && msg.panel === name) show(msg.data);
    else if (msg.type === 'close') closeSelf();
  });
  dock.addEventListener('click', () => {
    send({ from: name, type: 'dock' });
    setTimeout(closeSelf, 300);
  });
  send({ from: name, type: 'hello' });
}

main();
