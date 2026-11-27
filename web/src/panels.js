import { renderProblems, problemCount } from './problems.js';
import { renderTrace } from './inspector.js';

export const PANELS = {
  problems: {
    title: 'Problems',
    label: 'Problems: compiler errors and notation warnings',
    empty: [],
    render: (body, data, jump) => renderProblems(body, data, jump),
    badge(data) {
      const { errors, warnings } = problemCount(data);
      const parts = [];
      if (errors) parts.push(`${errors} error${errors > 1 ? 's' : ''}`);
      if (warnings) parts.push(`${warnings} warning${warnings > 1 ? 's' : ''}`);
      return { text: parts.join(' · '), tone: errors ? 'bad' : warnings ? 'warn' : '' };
    },
  },
  symbol: {
    title: 'Symbol',
    label: 'Symbol: the four layers of the symbol at the cursor',
    empty: { t: null, svg: '' },
    render: (body, data) => renderTrace(body, data.t, data.svg),
    badge: () => ({ text: '', tone: '' }),
  },
};

export function setBadge(el, info) {
  el.textContent = info.text;
  el.className = `dock-badge ${info.tone}`;
  el.hidden = !info.text;
}
