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

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
