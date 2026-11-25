# Symbol philosophy

Functor builds a symbol in four layers. Each layer has one job.

```mermaid
flowchart TB
  M["Meaning<br/><code>object vec: F</code><br/>what the symbol is"] --> S["Style<br/><code>set vec(style: arrow)</code><br/>how this kind looks in this document"]
  S --> C["Construction<br/><code>F.arrow</code>, <code>sum.circle</code><br/>which operations make the glyph"]
  C --> G["Glyph<br/>font outline, or a built outline from the cache"]
```

1. **Meaning.** You say what an object is. `object vec: F` says that `F` is a vector.
2. **Style.** One rule sets how a kind of object looks. `set vec(style: bold)` changes every vector in the document.
3. **Construction.** A recipe is a base and short operations: `theta.hat`, `in.not`, `integral.x2`. Each operation has one meaning.
4. **Glyph.** The compiler uses the font glyph when the font has it. If not, it builds the glyph from the recipe and keeps it in a cache. The cache also goes to disk, so a glyph is built one time only, not one time for each run.

## Rules

These are guidelines. The compiler gives a warning, not an error, when you go against one.

- Use at most three operations on one symbol. If you need more, define a `sym` with a clear name.
- Use the operation whose meaning fits. `hat` is a derived object, `tilde` is a related object, `not` is negation.
- Put meaning in the source and style in one rule. Do not write `F.arrow` in each place. Write `object vec: F` once.
- `deform` is the most general operation. In version 1 it changes existing symbols only. Phase 1 has the first kind: grow along one axis.
- Speed is part of the design. The compiler builds glyphs at compile time and keeps each recipe in a cache.

## Priorities

1. The glyph-building philosophy comes first.
2. No LaTeX compatibility. A tight and systematic symbol grammar. Good documentation and AI conversion help people move from LaTeX.
3. Personal use first. A possible link with Träumerei later.

## Phase 0 scorecard

Each line has a score from 0 to 5. 5 means that phase 0 does all that the principle asks for.

```mermaid
xychart-beta
  title "Phase 0 fit to the philosophy (0 to 5)"
  x-axis ["Meaning", "Style", "Construction", "Glyph", "Guidelines", "Speed", "Tight grammar", "Personal use"]
  y-axis 0 --> 5
  bar [4, 3, 3, 3, 5, 5, 4, 3]
```

| Principle | Score | What phase 0 does | Gap |
|:--|:-:|:--|:--|
| Meaning layer | 4 | `object vec:` declares meaning. Each operation has one written meaning, and the editor shows it. | `vec` is the only kind. Own kinds come in phase 1. |
| Style layer | 3 | `set vec(style: arrow\|bold)` changes all vectors from one rule. | `bold`, `bb`, `frak`, `script` and `up` are still operations in the recipe, so style and construction mix there. |
| Construction layer | 3 | 33 operations in 8 families: decorate, enclose, strike, repeat, reflect, style, magnify, fence. Operations work on a single symbol or on a group. `join(a, b)` makes one glyph from two symbols. | `deform`, `fuse` and `stretch` are not there yet. The compiler refuses them with a clear message. |
| Glyph layer | 3 | Font glyph first (∮, ∉, ∃, ∀, ħ). Other recipes are built and cached. | Built strokes have one fixed weight. They do not follow the weight of the font, and enclosures are tight on large operators. |
| Guidelines, not rules | 5 | More than three operations gives a warning. Unknown words give a warning with a hint. | None. |
| Speed | 5 | Worst new glyph 0.42 ms (gate 5 ms). Sample document 0.63 ms (gate 100 ms). | None. |
| Tight grammar | 4 | Short names only. No backslashes. Each error has a line, a column and a "Did you mean" hint. | `{n}` values do not work in math yet. |
| Personal use | 3 | The website runs in the browser with no server and keeps your file. Print to PDF works. | No Träumerei link yet. No page breaks for print. |

Total: 30 of 40.

## Structures scorecard

Matrices, vectors, cases and aligned equations use the same four layers as symbols. Before this change they scored 3 of 20. Each layer has a score from 0 to 5.

| Layer | Before | Now | What it does now | Gap |
|:--|:-:|:-:|:--|:--|
| Meaning | 1 | 4 | Kinds: `mat`, `vec`, `cases`, `binom`, `abs`, `norm`, `inner`, `floor`, `ceil`. | Own kinds wait for `kind` in phase 1. |
| Style | 1 | 4 | One rule for each kind: `fence`, `grow`, `body`, `word`. A fence can be any symbol. | Named matrices (`object mat: A`) have no style yet. |
| Construction | 0 | 4 | One `grid`. Fences are an operation family on any group, and `x2` doubles them. `wrap` takes any glyph. | The align point of a chain is always the first relation. |
| Layout | 1 | 4 | Columns, rows and fences that grow. Since phase 1, `stretch` uses `deform`, so thin strokes stay thin. | The align point of a chain is always the first relation. |

Total: 16 of 20 (15 before phase 1 deform).

### Decision: Functor draws its own math

The plan said that phase 0 would use browser MathML. Phase 0 draws math as SVG with its own layout instead. MathML can show only glyphs that are in a font. A built glyph such as `nabla.circle` has no code point, so MathML cannot show it. Own layout makes built glyphs equal to font glyphs. This fits priority 1.

### Key hiccups

- The font data from MathJax has no move command at the start of each path. Browsers did not draw the glyphs. The extractor now adds it.
- The double-struck letters (ℝ, ℕ) are in a separate table in the font package. The extractor now reads it.
- The first "Did you mean" check counted a swap of two letters as two changes. `alpah` got no hint. The check now counts a swap as one change.
- The proxy blocks jsDelivr and GitHub raw files. The font data comes from npm.

## Glyph cache scorecard (D2)

A recipe is built one time. The result and its errors go into the cache. The cache goes to disk, so the next run starts warm.

```mermaid
flowchart LR
  R["Recipe<br/><code>x.hat.box</code>"] --> M{"In memory?"}
  M -- yes --> U["Glyph + its errors"]
  M -- no --> B["Glyph builder"]
  B --> E["Entry: glyph + errors"]
  E --> U
  E -. new entry .-> D[("Disk cache<br/>CLI and desktop: glyphs.txt<br/>website: localStorage")]
  D -. next start, same key .-> M
```

- The cache keeps the errors of a recipe with its glyph. Before, a bad recipe showed its error only the first time. The second use came from the cache with no error, and `glyph_preview` showed the bad glyph.
- The file starts with a key. The key is a hash of the font data and of the glyph builder source. When the builder changes, old files do not load, so a stale glyph cannot show.
- The CLI uses `FUNCTOR_CACHE_DIR`, or the user cache folder. `--no-cache` turns it off. The desktop app uses its app cache folder. The website uses localStorage.

| Principle | Before | Now | What it does now | Gap |
|:--|:-:|:-:|:--|:--|
| Correct errors | 1 | 5 | Each use of a recipe gives its errors, at the right column. | None. |
| Keep across runs | 0 | 5 | CLI, desktop and website load the cache at start and write only new entries. | The cache has no size limit. It grows slowly, about 100 bytes for each recipe. |
| No stale glyphs | – | 5 | A key from the builder source and the font data. | A change to an unrelated part of `math.rs` also clears the cache. This is safe, and costs one cold run. |
| Speed | 5 | 5 | Disk load 0.04 ms for 22 glyphs. Sample document 0.9–1.7 ms after load. | Built glyphs cost less than 1 ms now, so the gain is small today. It grows with `deform`. |

Total: 20 of 20 for the cache. The four layers do not change: Meaning 4, Style 3, Construction 3, Glyph 3. The glyph gap is still stroke weight, not the cache.

## Phase 1 deform

`deform` changes the outline of an existing glyph. Version 1 has one kind: grow along one axis. It moves points and keeps the parts that have detail.

```mermaid
flowchart LR
  A["Glyph outline"] --> B["Cut into 48 bands<br/>along the axis"]
  B --> C["Score each band<br/>flat parts, many strokes, ends"]
  C --> D["Give the extra length<br/>to the plain bands"]
  D --> E["Split curves at band edges<br/>and move the points"]
  E --> F["New outline<br/>in the recipe cache"]
```

- A band with flat parts (serifs, the ends of a bracket, the tip of a brace) keeps its size.
- A band with more strokes than the plainest band (the loop of a script L) keeps its size.
- The 22% at each end of the glyph grows less, so terminals keep their shape.
- Each curve is split where it crosses a band edge. In one band the move is a plain scale, so the curve stays exact.

Where it is used:

- `tall` and `wide` are deform operations. `to.wide` is a long arrow.
- Fences with `grow: stretch` (the default) and the radical use deform. Only one fifth of the growth (as a power) is magnify, so large fences get a little heavier, as in good print.
- The cache key is the glyph and the size rounded up to 2%. Similar heights share one glyph.

| Principle | Score | What phase 1 deform does | Gap |
|:--|:-:|:--|:--|
| Deform is the most general operation | 3 | One kind of deform with two operations and the fence growth. | No bend, slant or taper yet. No deform on a group. |
| Existing symbols only (v1) | 5 | Deform takes any glyph: font, built or styled. | None. |
| Construction layer | 4 | 35 operations in 9 families. | `fuse` is not there yet. |
| Glyph layer | 4 | Stretched fences and letters keep their stroke weight. | The rule of a radical has a fixed weight. |
| Speed | 5 | About 0.2 ms for each new deformed glyph. Sample document: first compile 3.6 ms (was 1.6 ms), warm 0.9 ms. Gate 100 ms. | None. |

### Key hiccups

- Font outlines have no close command. The open end made a straight edge that did not follow the bands, so `⟩` was lopsided. Deform now closes each outline first.
- The loop of a script L has near vertical sides, so the first version stretched the loop. A count of strokes in each band fixed it.

## Symbol coverage (S1 to S3)

The audit found that the font package has 1,519 of 1,521 Unicode math symbols, but Functor named about 100. This change makes the whole font reachable and adds names, recipes and six operations.

```mermaid
flowchart LR
  F["Font package<br/>3,021 glyphs"] --> N["61 new names"]
  F --> R["81 new composites<br/>18 stacks"]
  O["6 new operations<br/>big, rot, ring, harpoon, alt, cal"] --> R
  S["stack(a, b)"] --> R
  D["Duplicate codes<br/>Ω ohm, Å, µ, ∆, K"] --> W["warning, then one form"]
```

### Operations: what stays out

Twenty operations were proposed. Six stay. Each one closes at least 3 gaps that no other operation can close.

| Operation | Family | Gaps it closes | Why no other operation can do it |
|:--|:--|:-:|:--|
| `big` | magnify | 9 | It changes the class to a large operator with limits |
| `rot` | reflect | 7 | `flip`, `flipv` and `turn` cannot point an arrow up |
| `ring` | decorate | 3 | No ring mark existed |
| `harpoon` | decorate | 3 | A half arrow head, on letters and on arrows |
| `alt` | style | 8 | The other common form of a symbol is a style choice |
| `cal` | style | 26 | TeX calligraphic and script are both used in one text |
| `stack(a, b)` | call | 18 | The vertical partner of `join` |

Left out: `long` (phase 1 `wide` does it), `ccw` and `cw` (one `rot` is enough), `inside` and `slash` (fewer than 3 gaps), `squig`, `dash`, `sans`, `mono`, `acute`, `grave`, four rare fences and `eval`.

### Duplicates

Some symbols have two Unicode codes, or two names for one look. The rule: one glyph has one name, and a recipe may explain that name (`E.flip` is `exists`). Tests refuse two names or two recipes for one glyph. The only exception is `perp` and `bot`, which share ⊥ but space differently. A typed second code (Ω ohm sign, Å angstrom sign, µ micro sign, ∆ increment, K kelvin sign, ⨯, ⨿, ⟂) gives a warning and shows the one form.

### Scorecard

| Principle | Before | Now | Gap |
|:--|:-:|:-:|:--|
| Meaning layer | 4 | 4 | `mono`, `epi`, `incl` are names, not kinds with a style rule yet |
| Style layer | 3 | 4 | `alt` and `cal` work in one place; there is no document rule such as `set greek(phi: alt)` yet |
| Construction layer | 4 | 5 | `big`, `rot` and `stack` reach the font glyph when it exists and build it when not |
| Glyph layer | 4 | 5 | The whole font package is reachable; typed Unicode gets a math class |
| Guidelines, not rules | 5 | 5 | Duplicate codes give a warning, not an error |
| Speed | 5 | 5 | Worst new glyph 0.89 ms (gate 5 ms). Sample document 0.91 ms warm (gate 100 ms) |
| Tight grammar | 4 | 4 | 41 operations, up from 35; tests stop duplicate names |
| Personal use | 3 | 3 | No change |

Total: 35 of 40 (32 before).

### Key hiccups

- The extractor imported the font files as code. Auto mode refused to run code from a downloaded package, so the extractor now reads the files as text. The 1,385 old glyphs came out byte for byte the same.
- The proxy blocks unicode.org, so the math class table comes from Unicode names (Python 15.1 data), not from the official MathClass file.
- The glyph table grew from 0.69 MB to 1.42 MB. The website download grows by about the same before compression.
- `…` (U+2026) stays out of the table, because Functor builds its dots from periods.
