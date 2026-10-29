# Triage

Stage 7. One row per request after the person has used the build: a bug, a change, or a new idea, in the person's own words. Rows are numbered T1, T2 and so on, and each commit that answers one names it ("Fix the timer reset (T4)"). `C4 Verify against hand-computed cases`.

Compare at least one result with a case worked out by hand. **Severity** is *blocks use*, *wrong*, or *wanted*. **Cost** is the agent's estimate. **Disposition** is *done*, *open*, *declined* (with the reason), or *try* (built on a branch and published as a TRY commit that says how to undo it).

| # | Request, in the person's words | Expected | What happened | Severity | Cost | Disposition |
| --- | --- | --- | --- | --- | --- | --- |
| T1 | "i dont want to expose all the latex and the coding stuff to the user, i want it to be stuff like a drop down menu or scratch programming in order to build the visualizations from terms in the formula. build a tab in the design log investigating how to hide all of the latex and coding in order to make this usable for kids in a 6th grade setting" | A sixth grader builds a visualisation from the parts of a formula without seeing LaTeX or the sheet. | The editor shows seven code surfaces (R2.1). Investigated on the journal page "5 · Sixth grade" with mockups K1–K4; nothing built yet. | wanted | large: five parts, in the build order on that page | done: built on the defaults (R2.4), all five parts; what was left out is R2.8 |
| T2 | "add in a few more baseline fprmulas to start from derived from the paper, and from more fields like linear algebra and calculus and discrete mathematics and probability and etc etc" | More starting formulas: from the paper, and from other fields. | 11 examples, one sixth-grade shelf and no headings. | wanted | medium: data, a menu heading per field, checks | done: 32 new (10 from the paper's sources, 3 shelf, 19 in seven fields), the Teacher menu grouped by field; R3.1 to R3.7 |
