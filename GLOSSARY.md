# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

### Augments

The sheet beside the formula that names its concepts and says how each one is drawn and changed. It keeps styling out of the formula itself.

Not: stylesheet

### Concepts

The ideas in a formula that an author marks, such as a slope or a step size. Each one has a number, a pattern and, within the colour budget, a colour of its own.

Not: tags

### Deutan

A view for readers with red-green colour vision deficiency. It uses blue and orange only, and keeps every pattern and number.

### Embed code

A piece of HTML that shows a live copy of the formula on another web page. The formula travels in the address, so nothing is uploaded.

### Mono

A view in ink only. Each concept is told apart by its pattern, its number and its label.

### Steps

A sequence that shows a formula one move at a time. Play runs them as an animation, and the GIF export records it.
