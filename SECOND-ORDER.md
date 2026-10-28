# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | finding | The untouched template passes `./scripts/check.sh` here. | Steps 1–8 pass in 15.6 s with 38 tests; step 9 (browser check) fails because Playwright is not installed (`ModuleNotFoundError: playwright`). `./scripts/setup.sh` has not been run. | Nothing yet; setup runs before the first build. | | open |
| R1.2 | finding | The default style pack's colours could serve as concept colours. | Under a deuteranopia simulation (Machado 2009, severity 1) `wallB` and `greenC` are ΔE 1.9 apart (49.5 in normal vision); `wallF`/`greenA` 2.7. Salmon and sage, the picture's main pair, collapse. | Concept colours come from a separate measured palette; the pack keeps the chrome. | a core test on pairwise distance per view (to write) | open |
| R1.3 | finding | Four dark concept hues at text contrast ≥ 4.5 on cream would stay distinct under deuteranopia. | harbor/plum fall from ΔE 48.9 to 4.6 and ochre/brick from 37.7 to 11.8; in grey all four sit between #595959 and #6A6A6A. At equal lightness only the blue–yellow axis survives. | Deutan view uses two hues × text/tint; every concept also carries a pattern and a number in every view. | journal FB-Q6; test to write | open |
| R1.4 | divergence | The TikZ figures would set labels in Helvetica. | This TeX Live install (basic scheme) has no `helvet`; pdflatex stopped on `phvr7t`. | Journal figures use Computer Modern Sans. | | done |
| R1.5 | worklist | The brief's Done means, Autonomy and style choices were written by the agent from the description. | They are proposals until answered (FB-Q1–FB-Q9). | Agent stops after the opening entries; no plan, no repository, no publish. | | person |
| R1.6 | divergence | FB-3 said values would be exact fractions. | Built as decimal fixed point (billionths): typed decimals add exactly, but one third is 0.333333333. A rational type would double the evaluator's size. | `value.rs`; noted in CLAUDE.md FB-3. | CLAUDE.md FB-3 | done |
| R1.7 | finding | The core could name a variable `env`. | `boundary.rs` flagged `env` as input/output in `doc.rs`. The scan held; the variable is now `scope`. | Renamed in four files. | boundary.rs (exists) | done |
| R1.8 | finding | My browser check expected six glyphs for `y = m x + b`. | The scrubbed slope draws as its value, 1.5, three glyphs, so the line draws eight. My hand count was wrong, not the tool. | Expectation corrected to eight, with the reason in the test. | tests/browser/interactions.py | done |
| R1.9 | finding | The page could write the document into the address on every draw. | WebKit throws after 100 `history.replaceState` calls in ten seconds, which stopped playback. | The page writes the address only when the document changes. | browser check, WebKit | done |
| R1.10 | divergence | Playback would advance one tick per animation frame. | WebKit's headless animation frames run slowly. | The page sends its clock and the core counts sixtieths of a second since play began. | `api.rs` `tick` | done |
| R1.11 | finding | Playwright's `wait_for_function` would work in the checks. | The page's CSP blocks string evaluation, so it fails. | Checks wait on selectors. | tests/browser/interactions.py | done |
| R1.12 | divergence | D11's release profile (opt-level s) would fit the budget. | 226 KB gzipped for the module alone; with opt-level z, one codegen unit, abort on panic and stripping, the first visit is 195 KB. | Cargo.toml; BUILDER-SPEC.md change table. | build.sh size budget | done |
| R1.13 | finding | A one-symbol concept would only match symbols standing alone. | `a` in `a^2` is a base of a power, not a row item, so it went unmarked. | One-symbol patterns also match bases of powers and primes, not of subscripted names (`w_k` stays a different name). | `sheet.rs` test | done |
| R1.14 | worklist | The five examples, their prose and every string in `data/strings.json` were written by the agent. | They are what a reader sees first. | Read them; FB-Q2 asks for formulas from your own courses. | | person |
| R1.15 | finding | The LaTeX nesting test covered the macro ordering rule. | The break that reverses ordering at a shared start left it passing: no test had two concepts starting on the same symbol. | Added `concepts_that_start_together_open_outer_first`; 12 of 12 breaks now caught. | tests/breaks.json | done |
| R1.16 | divergence | The first build matched the mockups. | Comparing each mockup with the running build found nine gaps: no legend, Mono names, Deutan preview, scrub arrows, eye per concept, SVG concept groups, plain LaTeX, space augmentation, clear background. | All nine built in this round; journal entry `2026-10-28-compare`. | | done |
| R1.17 | divergence | Every mocked feature would be in the first version. | Still missing: plots inside a formula, labels holding notation, connector arrows to the plot, toggles for definitions and plot marks, a two-handle limit, alt text copied with a PNG, decorations during a GIF morph, the step list in the editor pane. | Listed in the journal's comparison tab for a later round. | | open |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
| data/strings.json | every key (54 strings), written by the agent |
| data/examples.json | five examples: titles, formulas, sheets, prose |
