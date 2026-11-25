# Functor

Functor is a math typesetting language. The compiler is in `crates/functor-core`.

## Keep the reference current

The language changes fast. Each change to the language updates the reference in the same PR.

1. Change the compiler.
2. Run `cargo run -p functor-cli -- ref gen`. It writes the generated pages in `docs/reference/gen` from the compiler tables.
3. Update the hand pages in `docs/reference` that talk about the change. Remove text that is no longer true, such as "comes in phase 1".
4. Update `docs/reference/cheatsheet.fn`. It is the guide for AI models that do not know Functor. Keep it short.
5. Add a row to `docs/reference/latex.fn` when a LaTeX command has a new Functor form.
6. Run `cargo test --workspace`. The reference tests fail when a generated page is old, a page or example does not compile, a construct has no page, or the cheat sheet misses a name, an operation or a call.

CI fails when the generated pages are old, and warns when a PR changes the language but not `docs/reference`.

## Style

- Write docs in ASD-STE100 Simplified Technical English when you can.
- No code comments.
- Each PR reports how well it fits `docs/PHILOSOPHY.md`.
