# Plan

Stage 2, written by the agent from `BRIEF.md`. `B2 Require step-by-step justification`.

Approved in advance for round 1 (BRIEF.md Autonomy, 2026-10-28). Each decision in build order, with the reason for it and the test that shows it holds.

| # | Decision | Reason | Test that shows it holds |
| --- | --- | --- | --- |
| 1 | Vendor STIX Two Math (OFL) and convert it once to integer metrics and glyph paths (`tools/font/extract.py` → `data/font/stix.txt`), compiled into the core. | FB-Q5: layout in the core; no font to fetch; exports match the screen. | `font.rs` tests read the font's own numbers |
| 2 | Parse a LaTeX subset into a tree with preorder ids (`tex.rs`). | Concepts attach to any subtree (paper: half of augmentations sit on larger expressions). | `tex.rs` tests, errors named in words |
| 3 | Lay out in integer font units with TeX's spacing table and the MATH constants (`layout.rs`). | FB-1, FB-3: deterministic layout in the core. | `layout.rs` tests with hand-computed positions |
| 4 | Values in decimal fixed point (`value.rs`), formulas evaluated by `eval.rs`. | FB-3; no floats in the core. | values checked against table values |
| 5 | The augment sheet language (`sheet.rs`) holds concepts, controls, plots and steps, apart from the formula. | Paper R6, FB-4. | `sheet.rs` tests, line-numbered errors |
| 6 | Every concept carries a pattern and a number; four colours at most; three views (`palette.rs`). | FB-5 to FB-7, the brief's colour-blind requirement. | `palette.rs` tests; `tests/colour.rs` measures the palettes |
| 7 | The core draws the reader view as SVG (`render.rs`): labels at one height, braces, borders, tints, shapes, pointers, hover cards, plots with linked marks, morph frames. | FB-2: the page replays it; exports reuse it. | browser interactions; preview example |
| 8 | Exports: SVG and PNG from the same SVG; GIF encoded in the core (`gif.rs`); LaTeX with one macro per concept; embed and link carry the document in `#f=` (`export.rs`). | FB-8, FB-9. | `gif.rs` round trip through a reference decoder; LaTeX compiled by pdflatex by hand; browser link-to-embed check |
| 9 | One door: `api.rs` `act(name, value)` with the view model the page draws (`web/app.js`, `web/embed.html`). | Builder R4, R6. | `api.rs` tests; browser check in three engines |
| 10 | A first-visit size budget of 250 KB gzipped, checked in `scripts/build.sh`. | FB-11, browser performance. | the build fails over budget |

## Open questions for the person

Unanswered questions take the journal's defaults (DECISIONS.md, 2026-10-28): FB-Q2 (your teaching formulas), FB-Q6 to FB-Q10.

## Status

- [x] Approved in advance by the person on: 2026-10-28
