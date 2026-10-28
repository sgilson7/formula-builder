# Brief

Stage 1. The person's own words come first; the rest can be written by the agent from them and read by the person. `C1 Frame the problem and set the rules`, `B1 Establish context`.

## The description, word for word

> get yourself familiarized with the builder repository and clone it from https://github.com/sgilson7/builder-setting into this repo then remove the .git. You will be using builder setting to help constrain and direct your actions as you help act as as a video game  creator and co-designer / developer, with the goal of deploying it to a github pages so its freely browser accessible, with an emphasis on optimization for running in the browser.  Please also see https://sgilson7.github.io/projects/ for examples of other browser based tools / games built in builder like the PDF redactor, youtube to mp3, long time coming, extremely lifelike fishing  etc. Also see ptonkle.github.io for games built using builder by my good friend. First thing is I want you to do is interpret builder, and write an analysis of how you will use it to help you develop this tool and write into the first tab of the design journal, the design journal process is described in builder; when you write the design journal, use the local ape-race directories design journal as a representative of a good journaling process. you may access the other builder made games within the documents directory on this host machine. the tool I'd like to build is an online mathematical formula editor and builder than is designed for interactiability and maximum clarity in terms of concepts and relations between them. The paper I'd like to use as a basis for this work and use as the main citation is in the directory for your analysis. So 2nd tab of the design log should be your interpretation of the paper, and how you will mock up each finding / design decision / style of showing formulas within the online browser tool. the goal is interactability, so the formulas should be exportable for use in teaching materials, so stuff like as gifs if there is any movement, html embeds / iframes for other websites for interact formulas you can click on and mess with, and exportable also as png or as a latex formula. use the samm comm final png and the flaticon style as guidelines for style, and generate all 2d images in tikz using the tikz prompt, and make sure everything is colorblind safe / a colorblind mode exists for deuto and monochrome, so there should be ways for the formula to hold data / meaning in ways that are not just color. please start with the builder tab and the paper interpretation tab in the design log, then propose mockups and the design of the browser based tool in the third tab.

The paper in the directory is Head, Xie and Hearst, *Math Augmentation: How Authors Enhance the Readability of Formulas using Novel Visual Design Practices*, CHI '22, <https://doi.org/10.1145/3491102.3501932> (CC BY-NC-SA 4.0). It is the main citation.

## The tool

A formula editor that runs in a browser tab, for people who write teaching material with mathematics in it (teachers, textbook and blog authors, people making slides). The author types a formula in LaTeX, marks the parts that carry a concept, and adds what the paper calls math augmentations: colour together with a pattern and a number, labels, braces, borders, scrubbable values, sliders, links to a plot, and steps that show a derivation one move at a time. A reader can click, hover and drag the result. The author exports it as a PNG, an SVG, a GIF of the steps, LaTeX, or an embed for another web page. Desktop browsers first; a phone can read and play an embed.

## Done means

To be confirmed by the person (journal questions FB-Q1 to FB-Q9). Proposed:

- A LaTeX formula from the paper's examples (`y = mx + b`, the gradient-descent update, the real part of a complex number) parses, lays out and renders; a formula outside the supported subset gets an error that names the unsupported command.
- Each of the paper's sixteen kinds of augmentation that the plan keeps can be applied, and each has a core test.
- Every concept carries at least one channel besides colour in every view; Colour, Deutan and Monochrome views exist, and a core test fails if two concepts in the same view share colour and pattern.
- PNG, SVG, GIF, LaTeX and embed exports each produce a file a person can open, from the same document.
- The embed works inside an iframe on another site, with the formula carried in the address fragment and no request to another origin.
- `./scripts/check.sh` passes, and the page meets a load budget recorded in `MEASUREMENTS.md`.

## Decisions that stay with the person

- Which augmentations are in the first release, and which wait (FB-Q3).
- The example formulas shipped with the tool, and their wording.
- The tool's name and repository name (FB-Q1).
- Whether to publish, and where.
- The style pack: the Sam Comm picture and the flat icon look give the chrome; the concept colours are chosen for colour-vision safety, which can differ from the picture (FB-Q6).

## Accessibility

Every concept carries colour, an underline pattern (solid, dashed, dotted, zigzag, double, wave) and a number badge, and may carry a label. Three views: Colour, Deutan (a blue/orange palette with text and tint treatments) and Monochrome (ink only). Patterns and numbers survive every view and every export. Keyboard: every concept and every interactive value can be reached with Tab and changed with the arrow keys. Each export carries alt text built from the concept labels.

## Style

The default style pack (`styles/default/`, drawn from the Sam Comm picture) for chrome and art, in a flat icon look: thick rounded ink outlines, flat fills, no gradients. 2D images are TikZ, written to the TikZ figure prompt and rendered with `tools/art/`. Concept colours come from a separate, measured palette (FB-Q6).

## Words

Terms from the paper go into `GLOSSARY.md` before a string uses them: *math augmentation (maug)*, *concept*, *label*, *extent marker*, *linked selection*, *scrubbable*, *external control*, *details on demand*, *step*.

## Autonomy

Not yet decided: see FB-Q8. Until the person answers, the plan is **not** approved in advance, the agent stops after `PLAN.md`, and the agent does not create a repository or publish.

## Journal

`default`

## Allowed core dependencies

`serde`, `serde_json` (reference default). Add others only here, with the reason.
