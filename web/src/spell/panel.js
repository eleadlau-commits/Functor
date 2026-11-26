import { speller, LANGS } from './index.js';

export function spellPanel(button) {
  const box = document.createElement('div');
  box.className = 'spell-panel';
  box.hidden = true;
  box.setAttribute('role', 'dialog');
  box.setAttribute('aria-label', 'Spelling');

  const onRow = document.createElement('label');
  onRow.className = 'spell-row';
  const on = document.createElement('input');
  on.type = 'checkbox';
  on.addEventListener('change', () => speller.setOn(on.checked));
  onRow.append(on, ' Check the spelling of prose');

  const langRow = document.createElement('label');
  langRow.className = 'spell-row';
  const lang = document.createElement('select');
  for (const [k, v] of Object.entries(LANGS)) lang.append(new Option(v.label, k));
  lang.addEventListener('change', () => speller.setLang(lang.value));
  langRow.append('Dictionary ', lang);

  const head = document.createElement('div');
  head.className = 'spell-sub';
  const list = document.createElement('ul');
  list.className = 'spell-words';
  const hint = document.createElement('p');
  hint.className = 'spell-hint';
  hint.textContent = 'Math, code, labels and declarations are not checked. Right-click an underlined word, or put the cursor on it and press Ctrl+. (Cmd+. on a Mac), for suggestions. Alt+F7 goes to the next one.';
  const err = document.createElement('p');
  err.className = 'spell-error';

  box.append(onRow, langRow, head, list, hint, err);
  button.after(box);

  const draw = () => {
    on.checked = speller.on;
    lang.value = speller.lang;
    button.setAttribute('aria-pressed', String(speller.on));
    const words = [...speller.personal].sort((a, b) => a.localeCompare(b));
    head.textContent = words.length ? `My word list (${words.length})` : 'My word list is empty';
    list.replaceChildren(
      ...words.map((w) => {
        const li = document.createElement('li');
        const s = document.createElement('span');
        s.textContent = w;
        const x = document.createElement('button');
        x.type = 'button';
        x.className = 'link';
        x.textContent = 'Remove';
        x.setAttribute('aria-label', `Remove ${w}`);
        x.addEventListener('click', () => speller.remove(w));
        li.append(s, x);
        return li;
      }),
    );
    err.textContent = speller.error || '';
    err.hidden = !speller.error;
  };
  speller.onChange(draw);

  const close = (e) => {
    if (!box.contains(e.target) && e.target !== button) toggle(false);
  };
  const toggle = (open) => {
    box.hidden = !open;
    button.setAttribute('aria-expanded', String(open));
    if (open) {
      const r = button.getBoundingClientRect();
      box.style.top = r.bottom + 6 + 'px';
      box.style.right = Math.max(8, window.innerWidth - r.right) + 'px';
      document.addEventListener('pointerdown', close, true);
      on.focus();
    } else document.removeEventListener('pointerdown', close, true);
  };
  button.setAttribute('aria-expanded', 'false');
  button.addEventListener('click', () => toggle(box.hidden));
  box.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      toggle(false);
      button.focus();
    }
  });
}
