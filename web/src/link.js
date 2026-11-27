const EVENT = 'functor-panel';

export const native = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function openLink(id, onMessage) {
  if (native) {
    const { emit, listen } = await import('@tauri-apps/api/event');
    await listen(EVENT, (e) => {
      if (e.payload && e.payload.id === id) onMessage(e.payload);
    });
    return (msg) => emit(EVENT, { ...msg, id }).catch(() => {});
  }
  if (typeof BroadcastChannel === 'undefined') return () => {};
  const channel = new BroadcastChannel(`${EVENT}-${id}`);
  channel.onmessage = (e) => onMessage(e.data);
  return (msg) => channel.postMessage({ ...msg, id });
}

export const ICONS = {
  side: '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="2.5" width="13" height="11" rx="1.5"/><path d="M10 2.5v11" /><path class="fill" d="M10 2.5h3a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5h-3z"/></svg>',
  bottom: '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="2.5" width="13" height="11" rx="1.5"/><path d="M1.5 9.5h13" /><path class="fill" d="M1.5 9.5h13v2.5a1.5 1.5 0 0 1-1.5 1.5h-10a1.5 1.5 0 0 1-1.5-1.5z"/></svg>',
  pop: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M9 2.5h4.5V7"/><path d="M13.5 2.5 7.5 8.5"/><path d="M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"/></svg>',
  dock: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M7.5 8.5V4"/><path d="M7.5 8.5H12"/><path d="M7.5 8.5 13.5 2.5"/><path d="M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"/></svg>',
  close: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8"/></svg>',
};

export function iconButton(icon, title, act) {
  const b = document.createElement('button');
  b.className = 'dock-btn';
  b.dataset.act = act;
  b.title = title;
  b.setAttribute('aria-label', title);
  b.innerHTML = ICONS[icon];
  return b;
}
