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
};
