# Handoff

Stage 9, written by the agent. `C5 Consolidate in your own words`. A fresh session starts from this file, not from memory.

## What this is

Formula Builder is a browser formula editor for teaching material, built on Head, Xie and Hearst's *Math Augmentation* (CHI '22). An author types LaTeX, marks concepts in an augment sheet, and gets a reader view with colour, pattern and number per concept, labels, braces, scrubbable values, sliders, a linked plot and playable steps, in Colour, Deutan, Mono and Deutan-preview views. It exports PNG, SVG, GIF, LaTeX, an iframe embed and a link. It lives at <https://sgilson7.github.io/formula-builder/> (repository `sgilson7/formula-builder`, published from `gh-pages`).

## Rules that hold it up

| Rule | Guard |
| --- | --- |
| Every rule in the core; the page replays SVG and the view model | `crates/core/tests/boundary.rs` |
| No floats in the core: values are decimal fixed point, layout is integer font units | `boundary.rs` |
| Every concept has a pattern and a number; four colours at most | `sheet.rs` tests, `palette.rs` tests |
| Palettes readable and distinct for a deuteranope; the preview matches the simulation | `crates/core/tests/colour.rs` |
| First visit under 250 KB gzipped | `scripts/build.sh` |
| No off-origin request, no storage; the embed carries its document in `#f=` | CSP, `tests/browser/check.py`, `no_storage.rs` |

## Shape of the code

| Where | What |
| --- | --- |
| `tools/font/` | the vendored STIX Two Math, its licence, `extract.py` |
| `data/font/stix.txt` | integer metrics and outlines, compiled into the core |
| `crates/core/src/tex.rs` | LaTeX subset parser, node ids, canonical spellings |
| `layout.rs` | TeX-style layout |
| `value.rs`, `eval.rs` | fixed-point values, formula evaluation, functions and derivatives |
| `sheet.rs`, `doc.rs` | the augment sheet, concept matching, the document model |
| `palette.rs`, `render.rs` | views and colours; SVG reader view, plot, morph frames |
| `gif.rs`, `export.rs` | GIF encoder; LaTeX, fragment and file exports |
| `api.rs` | `start`, `act`, `gif`: the one door and the view model |
| `web/app.js`, `index.html`, `embed.html` | the page: draws and forwards |
| `data/strings.json`, `data/examples.json` | every reader-visible word, and the five examples |

## Commands

- `./scripts/check.sh`: ends `BUILDER CHECK PASSED`.
- `./scripts/count-tests.sh`: `82 passed, 0 failed, 0 ignored`.
- `python3 scripts/break_tests.py`: `12 of 12 breaks caught`.
- `python3 scripts/serve.py 8000` after `./scripts/build.sh`: the site on localhost.
- `cargo run -p tool_core --example preview -- DIR`: sample SVGs to look at.
- `./scripts/publish.sh`: refuses uncommitted changes, runs the check, pushes `dist/` to `gh-pages`.

## What will bite within the hour

- The page may hold no number other than 0 and 1 and may not call `Math`; sizes come from the core.
- Safari throws after 100 `history.replaceState` calls in ten seconds; write the address only when it changes (R1.9).
- The CSP blocks Playwright's `wait_for_function` strings; wait on selectors (R1.11).
- A scrubbed value draws as its number, so glyph counts change with values (R1.8).
- `env`, `time`, `io` and similar are banned identifiers in the core.

## The journal

`../formula-builder-journal/index.html`, its own repository, never pushed. A private copy of the opening entries was published as a claude.ai artifact. Newest entries: "Nine gaps found by the comparison, and built" and "The first version, and what it does" on the page "4 · Mockups vs reality".

## Open for the person

Every `SECOND-ORDER.md` row with status *person*, by number, with what the person has to decide or supply. `scripts/notebook_check.py` fails while one is missing.

- R1.14: read the five examples and the strings in `data/strings.json`; send formulas from your own courses (FB-Q2).
- R2.2: answer FB-Q11 to FB-Q15 on the journal page "5 · Sixth grade" (T1), read the student word table and menu words there (written by the agent), and arrange a session with sixth graders.
- R1.5: answered on 2026-10-28 for FB-Q1, FB-Q3, FB-Q4 and FB-Q5; FB-Q2 and FB-Q6 to FB-Q10 took their defaults.

## Mistakes so far

- Hand-computed expectations: my glyph count missed the scrubbed number (R1.8).
- Tests that could not fail: the LaTeX nesting test did not exercise the ordering rule (R1.15); a new test covers it.
- Browser differences: WebKit's `replaceState` limit and slow animation frames (R1.9, R1.10); playback now runs on the page's clock.
- Builder's scans held: a core variable named `env` (R1.7).
- Scope against the mockups: nine gaps built, eight listed as open (R1.16, R1.17).

## What passed

2026-10-28, commit `8d4f300`: `./scripts/check.sh` ended `BUILDER CHECK PASSED` (browser check in chromium, firefox, webkit); `./scripts/count-tests.sh` printed `82 passed, 0 failed, 0 ignored`; first visit 199 KB gzipped.

## The next action

Use the live page on one case worked by hand (slope 1.5, intercept 1 puts the line through (2, 4)), then answer R1.14.
