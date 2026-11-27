window.functorNotes = function functorNotes(root) {
  const docs = root.matches && root.matches('.fn-doc') ? [root] : [...root.querySelectorAll('.fn-doc')];
  for (const doc of docs) {
    const sides = [...doc.querySelectorAll('.fn-side:not(.fn-side-inline)')];
    for (const s of sides) s.style.transform = '';
    const gap = 4;
    let floor = -Infinity;
    const moves = [];
    for (const s of sides) {
      if (getComputedStyle(s).position !== 'absolute') continue;
      const r = s.getBoundingClientRect();
      const shift = Math.max(0, floor + gap - r.top);
      if (shift > 0.5) moves.push([s, shift]);
      floor = r.bottom + shift;
    }
    for (const [s, shift] of moves) s.style.transform = 'translateY(' + shift.toFixed(1) + 'px)';
  }
};

window.functorRefs = function functorRefs(root) {
  const notes = () => window.functorNotes(root);
  notes();
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(notes);
  let notesTimer = 0;
  window.addEventListener('resize', () => {
    clearTimeout(notesTimer);
    notesTimer = setTimeout(notes, 60);
  });
  let pop = null;
  let back = null;

  function hide() {
    if (pop) {
      pop.remove();
      pop = null;
    }
  }

  function targetOf(a) {
    const id = decodeURIComponent((a.getAttribute('href') || '').slice(1));
    return id ? root.querySelector('[id="' + id.replace(/"/g, '\\"') + '"]') : null;
  }

  function kindOf(t) {
    if (t.matches('.fn-equation')) return 'Equation';
    if (/^H[1-6]$/.test(t.tagName)) return 'Section';
    if (t.matches('figure')) return 'Figure';
    const n = t.querySelector('.fn-env-name');
    return n ? n.textContent.trim() : 'Target';
  }

  function show(a) {
    const t = targetOf(a);
    if (!t || t.contains(a)) return;
    hide();
    const doc = a.closest('.fn-doc');
    pop = document.createElement('div');
    pop.className = 'fn-pop' + (root.classList.contains('fn-dark') || (doc && doc.classList.contains('fn-dark')) ? ' fn-dark' : '');
    pop.setAttribute('role', 'tooltip');
    if (doc) {
      for (const c of doc.classList) if (c.startsWith('fn-style-')) pop.classList.add(c);
    }
    if (!t.querySelector('.fn-env-name')) {
      const head = document.createElement('div');
      head.className = 'fn-pop-kind';
      head.textContent = kindOf(t);
      pop.appendChild(head);
    }
    const parts = /^H[1-6]$/.test(t.tagName) ? [t, t.nextElementSibling].filter(Boolean) : [t];
    for (const p of parts) {
      const c = p.cloneNode(true);
      c.removeAttribute('id');
      c.querySelectorAll('[id]').forEach((n) => n.removeAttribute('id'));
      c.querySelectorAll('.fn-side, .fn-num').forEach((n) => n.remove());
      pop.appendChild(c);
    }
    document.body.appendChild(pop);
    const r = a.getBoundingClientRect();
    const w = pop.offsetWidth;
    const h = pop.offsetHeight;
    const left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8));
    const below = r.bottom + 8;
    const top = below + h > window.innerHeight - 8 && r.top - h - 8 > 8 ? r.top - h - 8 : below;
    pop.style.left = left + 'px';
    pop.style.top = top + 'px';
  }

  function flash(t) {
    t.classList.remove('fn-flash');
    void t.offsetWidth;
    t.classList.add('fn-flash');
  }

  function showBack(a) {
    if (back) back.remove();
    back = document.createElement('button');
    back.type = 'button';
    back.className = 'fn-back';
    back.textContent = '↩ Back to ' + (a.textContent || 'text').trim();
    back.addEventListener('click', () => {
      a.scrollIntoView({ block: 'center', behavior: 'smooth' });
      flash(a);
      back.remove();
      back = null;
    });
    document.body.appendChild(back);
  }

  root.addEventListener('mouseover', (e) => {
    const a = e.target.closest && e.target.closest('a.fn-ref');
    if (a && root.contains(a) && matchMedia('(hover: hover)').matches) show(a);
  });
  root.addEventListener('mouseout', (e) => {
    const a = e.target.closest && e.target.closest('a.fn-ref');
    if (a && !a.contains(e.relatedTarget)) hide();
  });
  root.addEventListener('focusin', (e) => {
    const a = e.target.closest && e.target.closest('a.fn-ref');
    if (a) show(a);
  });
  root.addEventListener('focusout', hide);
  root.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('a.fn-ref');
    if (!a) return;
    const t = targetOf(a);
    if (!t) return;
    e.preventDefault();
    hide();
    t.scrollIntoView({ block: 'center', behavior: 'smooth' });
    flash(t);
    showBack(a);
  });
  window.addEventListener('scroll', hide, { passive: true, capture: true });
  window.functorHere(root);
};

window.functorHere = function functorHere(root) {
  const bar = document.createElement('nav');
  bar.className = 'fn-here';
  bar.setAttribute('aria-label', 'Current section');
  bar.hidden = true;
  document.body.appendChild(bar);
  let key = '';
  let frame = 0;

  function scroller() {
    for (let el = root; el && el !== document.body && el !== document.documentElement; el = el.parentElement) {
      const o = getComputedStyle(el).overflowY;
      if ((o === 'auto' || o === 'scroll') && el.scrollHeight > el.clientHeight) return el;
    }
    return null;
  }

  function trail(heads, top) {
    let at = -1;
    for (let i = 0; i < heads.length; i++) {
      if (heads[i].getBoundingClientRect().top < top + 1) at = i;
      else break;
    }
    if (at < 0) return [];
    const stack = [];
    for (let i = 0; i <= at; i++) {
      const lv = +heads[i].tagName[1];
      while (stack.length && +stack[stack.length - 1].tagName[1] >= lv) stack.pop();
      stack.push(heads[i]);
    }
    return stack;
  }

  function label(h) {
    const c = h.cloneNode(true);
    c.querySelectorAll('.fn-side, .fn-num').forEach((n) => n.remove());
    c.querySelectorAll('[id]').forEach((n) => n.removeAttribute('id'));
    const num = h.querySelector('.fn-num');
    return { num: num ? num.textContent.trim() : '', nodes: [...c.childNodes] };
  }

  function fill(path) {
    bar.textContent = '';
    path.forEach((h, i) => {
      if (i) {
        const sep = document.createElement('span');
        sep.className = 'fn-here-sep';
        sep.setAttribute('aria-hidden', 'true');
        sep.textContent = '\u203A';
        bar.appendChild(sep);
      }
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'fn-here-item';
      const { num, nodes } = label(h);
      if (num) {
        const n = document.createElement('span');
        n.className = 'fn-here-num';
        n.textContent = num;
        b.appendChild(n);
      }
      const t = document.createElement('span');
      t.className = 'fn-here-text';
      t.append(...nodes);
      b.appendChild(t);
      b.addEventListener('click', () => h.scrollIntoView({ block: 'start', behavior: 'smooth' }));
      bar.appendChild(b);
    });
  }

  function update() {
    frame = 0;
    const doc = root.matches && root.matches('.fn-doc') ? root : root.querySelector('.fn-doc');
    const heads = doc ? [...doc.querySelectorAll('h2, h3, h4, h5, h6')].filter((h) => !h.closest('.fn-pop, .fn-side')) : [];
    const box = scroller();
    const view = box ? box.getBoundingClientRect() : { top: 0, left: 0, right: window.innerWidth };
    const top = Math.max(view.top, 0);
    const path = heads.length ? trail(heads, top + 48) : [];
    if (!path.length) {
      bar.hidden = true;
      key = '';
      return;
    }
    const next = path.map((h) => h.textContent).join('\u0000') + path.length;
    if (next !== key || bar.hidden) {
      key = next;
      bar.className = 'fn-here' + (root.classList.contains('fn-dark') || doc.classList.contains('fn-dark') ? ' fn-dark' : '');
      for (const c of doc.classList) if (c.startsWith('fn-style-')) bar.classList.add(c);
      fill(path);
      bar.hidden = false;
    }
    const r = doc.getBoundingClientRect();
    const left = Math.max(r.left, view.left);
    const right = Math.min(r.right, view.right);
    bar.style.top = top + 'px';
    bar.style.left = left + 'px';
    bar.style.width = Math.max(0, right - left) + 'px';
  }

  function later() {
    if (!frame) frame = requestAnimationFrame(update);
  }

  window.addEventListener('scroll', later, { passive: true, capture: true });
  window.addEventListener('resize', later);
  new MutationObserver((records) => {
    if (records.some((m) => !bar.contains(m.target) && m.target !== document.body)) later();
    else if (records.some((m) => m.target === document.body && [...m.addedNodes, ...m.removedNodes].some((n) => n !== bar && !(n.classList && (n.classList.contains('fn-pop') || n.classList.contains('fn-back')))))) later();
  }).observe(root, { childList: true, subtree: true, attributes: true, attributeFilter: ['class'] });
  later();
};
