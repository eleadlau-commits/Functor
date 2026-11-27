export function problemCount(diags) {
  const errors = diags.filter((d) => d.severity === 'error').length;
  return { errors, warnings: diags.length - errors };
}

export function renderProblems(root, diags, onJump) {
  if (!diags.length) {
    const empty = document.createElement('p');
    empty.className = 'dock-empty';
    empty.textContent = 'No problems. Errors and notation warnings show here.';
    root.replaceChildren(empty);
    return;
  }
  const list = document.createElement('ul');
  list.className = 'list';
  list.append(
    ...diags.map((d) => {
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
      const jump = () => onJump(d.line, d.col);
      li.addEventListener('click', jump);
      li.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') jump();
      });
      return li;
    }),
  );
  root.replaceChildren(list);
}
