import assert from 'node:assert/strict';
import { proseWords } from '../web/src/spell/scan.js';

const words = (src) => proseWords(src).map((w) => w.word);
const cases = [
  ['Plain text here.', ['Plain', 'text', 'here']],
  ['A $alpah + x.hat$ b and $$ y $$ end.', ['and', 'end']],
  ['---\nfunctor: 1\ntitle: Teh notes\nauthor: Zed Qx\n---\n\nHi there.', ['Teh', 'notes', 'Hi', 'there']],
  ['object vec: v\nsym csum = sum.circle\nset mat(fence: brack)\nlet n = 3\n\nWords.', ['Words']],
  ['```fn\nbad wrods\n```\nGood.', ['Good']],
  ['$$\nfrac(a, b) "iff"\n$$ {#eq:x}\nAfter.', ['After']],
  ['See @eq:newton and @thm:main [@knuth84] for {n} in `code`.', ['See', 'and', 'for', 'in']],
  ['::: theorem {#thm:a title="Closed sums"}\nBody text.\n:::', ['Closed', 'sums', 'Body', 'text']],
  ['A [link](https://example.com/wrods) and ![alt](plot.svg) at https://x.org/teh.', ['link', 'and', 'alt', 'at']],
  ['NASA and LaTeX and x2 and snake_case and file.fn ok.', ['and', 'and', 'and', 'and', 'ok']],
  ['Don’t stop, it’s the symbol’s glyph.', ['Don’t', 'stop', 'it’s', 'the', 'symbol’s', 'glyph']],
  ['Cost \\$5 and \\@home.', ['Cost', 'and', 'home']],
  ['Inline $a +\nb$ spans lines.', ['Inline', 'spans', 'lines']],
  ['Open $x\n\nNext para.', ['Open', 'Next', 'para']],
  ['| `theta.hat` | $theta.hat$ | decorate |', ['decorate']],
  ['A note.^[Margin wrods here.]', ['note', 'Margin', 'wrods', 'here']],
];
for (const [src, want] of cases) assert.deepEqual(words(src), want, src);

const src = 'Hello wrold.';
for (const w of proseWords(src)) assert.equal(src.slice(w.from, w.to), w.word);
console.log(`spell scan test passed (${cases.length} cases)`);
