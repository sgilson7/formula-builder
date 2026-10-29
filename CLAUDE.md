# Instructions for a coding agent in a Builder project

`AGENTS.md` holds the same rules for agents that read that file instead.

## Where code goes

1. Read `BRIEF.md`, `BUILDER-SPEC.md` and `GLOSSARY.md` before changing code.
2. The core (`crates/core`) owns every application decision and performs no input or output. Scripts and tools that need a rule read the core's output; they do not recompute it.
3. The shim (`crates/shim`) translates values and decides no application behaviour.
4. The page (`web/`) draws what the core returns and does not reproduce a core rule. The core describes the screen (`ui`) and takes settings, screens and flags through one function (`act`). Presentation numbers go in `data/*.json`; shaders go in `data/shaders/` and take rule values as uniforms.

## How to work

5. Run `cargo test` after each meaningful core change.
6. For each new behaviour test, add a row to `tests/breaks.json` and run `python3 scripts/break_tests.py`: the test must fail with its behaviour broken. Work expected values out by hand; never compare a value with the function under test. When a test needs a workaround to pass, the workaround is the finding: record it.
7. Keep `SECOND-ORDER.md` while you work: one row per assumption, surprise, divergence from the plan, or item the person must do, written when you notice it. It is how the person sees what you decided alone. A row that produced a rule names where the rule now lives; a row with status *person* is named in `HANDOFF.md`.
8. A scripted edit that replaces text asserts that it matched. A count in a document comes from `./scripts/count-tests.sh`.
9. Do not deploy unless `BRIEF.md`'s Autonomy paragraph says you may. Deploying is otherwise the person's decision (stage 8).
10. Before handoff, run `./scripts/check.sh` and record the result in `HANDOFF.md`.

## Words

11. Use each term in the sense `GLOSSARY.md` gives it, and never a word from its `Not:` line. A new term gets an entry before it appears in a string.
12. Reader-visible text follows `TONE.md`. A string that looks wrong is reported in `SECOND-ORDER.md`, not repaired. A string you write yourself is listed at the foot of `SECOND-ORDER.md` for the person to read. A list you generate that a reader will see (names, labels, word lists) is a worklist row for the person.

## The design journal

13. Keep the design journal unless `BRIEF.md`'s Journal section says `none`. Start it with `python3 tools/journal/new_journal.py`; it lives beside the project (by default `../<this folder>-journal/`) as its own repository and is never pushed. `tools/journal/ENTRIES.md` says what each kind of entry holds.
14. Before writing `PLAN.md`, write the journal's opening entry: read Builder and the brief back to the person, with numbered rules and what holds each, the question prefix, where you stop, and your questions. When the person answers, each rule that applies to every later session goes under "Rules agreed for this project" below, and each rule the core can enforce becomes a test.
15. Every decision put to the person, visible change, divergence from the plan and deploy gets a journal entry when it happens, citing its rows (T4, R2.3, XX-Q5). Never remove an entry. `scripts/journal_check.py` checks the form; commit in the journal's repository after each change.

## Rules agreed for this project

Agreed 2026-10-28 (journal entry `2026-10-28-agreed`; reasons in the journal's opening entry).

- FB-1, FB-2: parsing, layout, hit-testing, concepts, palettes, values, steps and every export encoder live in the core; the page replays the SVG and draw data the core returns.
- FB-3: no floats in the core; values use decimal fixed point (`value.rs`), layout uses integer font units.
- FB-4: the formula source holds no styling; augments live in the sheet.
- FB-5, FB-6: every concept carries a pattern and a number in every view; `crates/core/tests/colour.rs` holds the colour distances.
- FB-7: at most four concept colours; later concepts get pattern and number only.
- FB-8, FB-9, FB-10: exports come from the core; no off-origin request; no browser storage; the embed carries its document in the address fragment.
- FB-11: the build fails when the gzipped site passes the size budget in `scripts/build.sh`.
- FB-12: 2D images are TikZ.
- FB-14: cite Head, Xie and Hearst (CHI '22, doi:10.1145/3491102.3501932) in the README and the about panel; copy none of its figures.
- FB-15 (agreed 2026-10-28, journal `2026-10-28-kids-agreed`): the Explore and Build modes show no LaTeX and no sheet; the code shows only in the Teacher mode with its switch on. A key on the tiles goes in only where it fits. `the_code_shows_only_in_the_teacher_mode_with_its_switch_on`, `a_key_that_does_not_fit_is_refused`, and the browser check.
- Publishing to `sgilson7/formula-builder` is granted (DECISIONS.md, 2026-10-28).

## Stopping

Write the journal's opening entry, then `PLAN.md` from the brief, and stop until the person has read it, unless `BRIEF.md`'s Autonomy paragraph approves the plan in advance. When a rule in `BUILDER-SPEC.md` seems to block the work, stop and say so in a worklist row; do not loosen a check to make it pass. When a request is refused on safety grounds, leave the feature out and say so; do not reword the request to get past the refusal.
