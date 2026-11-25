# functor.js

Typeset [Functor](https://github.com/eleadlau-commits/Functor) math on any web page. The Functor compiler runs in the browser as WebAssembly. Nothing goes to a server.

## Use

```html
<script src="https://cdn.jsdelivr.net/npm/functor-js@0.1.0/functor.js"></script>

<p>Each <fn-math>x in R.bb</fn-math> has <fn-math>x^2 ge 0</fn-math>.</p>
<fn-math display>sum_(k = 1)^n k = frac(n(n + 1), 2)</fn-math>
<fn-doc>Let $A = mat(1, 2; 3, 4)$. Then $det A = -2$.</fn-doc>
```

| You write | You see |
|:--|:--|
| `<fn-math>...</fn-math>` | inline math |
| `<fn-math display>...</fn-math>` | display math |
| `<fn-doc>...</fn-doc>` | text with `$...$` and `$$` math |
| a code block with the class `language-fn`, `language-functor` or `language-fnmath` | a rendered block |
| inline code that starts with `fn:` | inline math |

The math takes the colour and size of the text around it, so dark mode works. The text fonts are in the script. Errors show the source in red, with the message on hover. When the page changes, the math updates.

## JavaScript

```js
await Functor.render('x^2')
await Functor.render(src, { display: true })
await Functor.compile(src, widthPt)
Functor.renderAll(element)
```

Add `data-auto="false"` to the script tag to stop the scan of code blocks.

## Files

| File | Job |
|:--|:--|
| `functor.js` | the script, with the CSS and the fonts (about 230 KB) |
| `functor.wasm` | the compiler (3 MB, 1.2 MB with gzip) |
| `functor-wasm.js` | the compiler as a script, for pages that block `fetch` |

The page must allow WebAssembly. Language reference: [eleadlau-commits.github.io/Functor/reference](https://eleadlau-commits.github.io/Functor/reference/help/web.html).

MIT licence. The Latin Modern fonts use the GUST Font License.

## Build

From the repository root: `node scripts/build-functor-js.mjs`. The result is in `js/dist`. Test: `node scripts/functor-js-smoke.mjs`. The demo page is `js/demo/index.html`.
