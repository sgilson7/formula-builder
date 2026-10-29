# Formula Builder

A formula editor for teaching material. Type a formula in LaTeX, mark the concepts in it, and give each concept the augmentations that Head, Xie and Hearst catalogued in *Math Augmentation*: a colour with a pattern and a number, labels, braces, borders, tints, pointers, extra space, values a reader can drag or type, sliders, a plot whose marks follow the formula, and steps that play a derivation one move at a time. Export the result as a PNG, an SVG, a GIF of the steps, LaTeX, or an embed for another web page.

**Use it:** <https://sgilson7.github.io/formula-builder/>

Everything runs in your browser tab. Nothing is uploaded, and the page keeps nothing between visits: save a document as a file, or keep its link, which carries the whole document after the `#`.

## Three modes, for a sixth-grade class

The editor opens in **Build**, made for students with no LaTeX and no code on screen:

- **Shelf:** start from a formula in the sixth-grade standards, such as the area of a triangle, d = r × t, or 3(2 + x).
- **Tiles and keys:** change the formula with a keypad. A key goes in only where it fits, so a formula cannot break.
- **Cards:** tap a symbol (tap again for a bigger part) and finish sentences with menus: what it is, its unit, whether readers drag it or use a slider, and from what to what.
- **Blocks:** build the steps from blocks such as *show line 2*, *spotlight the 3s* and *cross out*, and press Play.

**Explore** is for reading: hover, drag, slide and play, with nothing to change. A link a student shares opens there. **Teacher** has everything, every export, and a switch that shows the LaTeX and the sheet beside the cards and blocks.

The Teacher mode's examples menu has 43 starting formulas under nine headings: the sixth-grade shelf, formulas from the documents the paper analysed (the rendering equation, the discrete Fourier transform, Bayes' theorem, the Bellman equation and more), algebra, calculus, linear algebra, discrete mathematics, probability and statistics, machine learning and physics.

## Numbers that work themselves out

Under each formula that defines a value, a worked line writes it again with the reader's numbers in place and its result: move the sliders of the mean and `m = (4 + 7 + 10)/3 = 7` becomes `(6 + 7 + 10)/3 ≈ 7.667`. The numbers slide and roll into their new places; with reduced motion turned on in the system settings they change at once. **Full screen** shows the reader alone, for showing a formula to a room; Esc leaves it.

## Colour-vision views

Colour is the paper's most common augmentation, so every concept also carries an underline pattern (solid, dashed, dotted, wave, double) and a number, in every view and every export.

| View | What it shows |
| --- | --- |
| Colour | four concept colours, each with at least 4.5:1 contrast on the canvas |
| Deutan | blue and orange only, as text or as a tint, the one hue axis a deuteranope keeps |
| Mono | ink only; each concept's name sits beside its number |
| Deutan preview | the Colour view as a deuteranope would see it, for checking |

`crates/core/tests/colour.rs` measures the palettes under a deuteranopia simulation (Machado et al. 2009) and in contrast ratio. A fifth concept gets a pattern and a number but no colour of its own: the paper's authors spoke of a "budget" of colour.

## The augment sheet

The formula holds no styling. The sheet beside it does, so the LaTeX stays readable (the paper's recommendation R6):

```text
concept slope: m
  say how far y rises for one step to the right in x
  label above
  scrub -3 .. 3 step 0.5 = 1.5
  limit -2 .. 2
concept intercept: b
  brace below
  slider -4 .. 4 step 0.5 = 1
plot y over x from -5 to 5
  mark slope rise x = 1
  mark intercept intercept
step Start
  focus slope
```

A concept's pattern is LaTeX matched against every formula; it can be one symbol (`m`, `\alpha`, `w_k`) or a run of terms (`\nabla f(w_k)`). Click part of a formula in the reader view to select it, click again to select a larger part, and add it as a concept. The full list of sheet commands is under "Sheet reference" in the editor.

## Embedding

**Embed code** gives an `<iframe>` pointing at `embed.html` with the document in the fragment. The embedded copy keeps hovering, dragging, sliders, the plot and the steps, and sends nothing to any server.

## How it is built

Formula Builder follows [Builder](https://github.com/sgilson7/builder-setting). Every rule lives in a Rust core with no input, output, floats or clocks: the LaTeX parser, a TeX-style layout in integer font units, the concepts and their palettes, decimal fixed-point values, the plot, the step morphs, and the PNG/SVG/GIF/LaTeX/embed encoders. A WebAssembly shim passes values through, and the page draws the SVG the core returns. `./scripts/check.sh` runs the core tests, the boundary tests, the release build with a size budget (250 KB gzipped for a first visit) and a browser check in Chromium, Firefox and WebKit.

The math font is STIX Two Math (SIL Open Font License 1.1), converted once into integer metrics and outlines by `tools/font/extract.py`.

## Citation

The design follows the inventory and recommendations of:

> Andrew Head, Amber Xie, and Marti A. Hearst. 2022. Math Augmentation: How Authors Enhance the Readability of Formulas using Novel Visual Design Practices. In *CHI Conference on Human Factors in Computing Systems (CHI '22)*. ACM. <https://doi.org/10.1145/3491102.3501932>

No figure from the paper is reproduced here.

## Licence

Code: MIT, see `LICENSE`. Font: SIL Open Font License 1.1, see `tools/font/OFL.txt` and `LICENSES.md`.
