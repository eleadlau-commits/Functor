# Functor in Claude

A Chrome extension. It shows Functor code blocks in Claude chats (claude.ai) as typeset math. The Functor compiler runs inside the extension as WebAssembly. Nothing goes to a server.

![Functor blocks in a chat](screenshot.png)

## Install

1. Get the extension folder:
   - From a release: download `functor-extension-<version>.zip` from [Releases](https://github.com/eleadlau-commits/Functor/releases) and unzip it.
   - From CI: open the latest **Browser extension** run in the Actions tab, download the `functor-extension` artifact, and unzip it.
   - Or build it: `node scripts/build-extension.mjs` (needs Rust with the `wasm32-unknown-unknown` target). The result is in `extension/dist`.
2. Open `chrome://extensions` and turn on **Developer mode**.
3. Click **Load unpacked** and select the folder.
4. Reload the claude.ai tab.

To update, replace the folder and click the reload arrow on the extension card.

## Use

| You write | You see |
|:--|:--|
| a code block with the language `fn` or `functor` | a rendered block |
| a code block with the language `fnmath` | display math, always |
| inline code that starts with `fn:`, for example `` `fn: theta.hat` `` | inline math |

A `fn` block with no `$` and no heading is display math. A `fn` block with `$` is a small document: text with inline and display math. Lines at the top that start with `set`, `sym`, `object`, `let` or `use` stay as rules. The extension adds the `functor: 1` header when the block has none.

Each block has a **Show source** button. The toolbar icon turns the extension off and on for all tabs.

## Ask Claude to write Functor

Claude does not know Functor. Paste the "Short version for instructions" block from the [cheat sheet](https://eleadlau-commits.github.io/Functor/reference/cheatsheet.html) into your Claude project instructions or your profile preferences. When Claude needs more, give it the whole cheat sheet ([source](https://eleadlau-commits.github.io/Functor/reference/cheatsheet.fn)).

## Same code as functor.js

The extension and [functor.js](../js/README.md) share one renderer: `js/src/core.js`. The build puts it into `content.js`. A fix there fixes both.

## Limits

- Other people see the rendered math only when they also have the extension.
- The extension reads the page structure of claude.ai. When claude.ai changes its code blocks, the extension can stop finding them.
- Chrome and other Chromium browsers only for now.

## Test

`node scripts/extension-smoke.mjs` loads the built extension in Chromium on a copy of a chat page and checks the result. Set `SHOTS=dir` for a screenshot and `DARK=1` for dark mode.
