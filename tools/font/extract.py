#!/usr/bin/env python3
"""Turn the vendored STIX Two Math font into the integer table the core reads.

Run once when the glyph set changes:

    python3 tools/font/extract.py        # writes data/font/stix.txt

The font (SIL Open Font License 1.1) is pinned by tools/font/SHA256SUMS. The
output holds, in font units (1000 per em, y up):

    k NAME VALUE                       a MATH-table constant
    g KEY ADV XMIN YMIN XMAX YMAX ITAL PATH
                                       a glyph: KEY is a hex code point, or
                                       a variant's glyph name; PATH is SVG
    v HEX KEY KEY ...                  a code point's vertical size variants,
                                       smallest first
The core embeds this file at compile time and never parses a font itself.
"""
import hashlib
import sys
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FONT = HERE / "STIXTwoMath.otf"
OUT = ROOT / "data" / "font" / "stix.txt"

CONSTANTS = [
    "AxisHeight", "FractionRuleThickness", "FractionNumeratorDisplayStyleShiftUp",
    "FractionDenominatorDisplayStyleShiftDown", "FractionNumeratorShiftUp", "FractionDenominatorShiftDown",
    "FractionNumDisplayStyleGapMin", "FractionDenomDisplayStyleGapMin", "FractionNumeratorGapMin",
    "FractionDenominatorGapMin", "SuperscriptShiftUp", "SuperscriptShiftUpCramped", "SubscriptShiftDown",
    "SuperscriptBottomMin", "SubscriptTopMax", "SubSuperscriptGapMin", "ScriptPercentScaleDown",
    "ScriptScriptPercentScaleDown", "RadicalVerticalGap", "RadicalDisplayStyleVerticalGap",
    "RadicalRuleThickness", "RadicalExtraAscender", "UpperLimitGapMin", "LowerLimitGapMin",
    "UpperLimitBaselineRiseMin", "LowerLimitBaselineDropMin", "AccentBaseHeight",
    "DelimitedSubFormulaMinHeight", "DisplayOperatorMinHeight", "SpaceAfterScript",
    "OverbarVerticalGap", "OverbarRuleThickness", "UnderbarVerticalGap", "UnderbarRuleThickness",
]


def codepoints():
    cps = set(range(0x20, 0x7F))                       # ASCII
    cps |= set(range(0x1D434, 0x1D468))                # italic A-Z, a-z
    cps |= set(range(0x1D6E2, 0x1D71C))                # italic Greek
    cps |= set(range(0x0391, 0x03AA)) | set(range(0x03B1, 0x03CA))  # upright Greek
    cps |= set(range(0x1D538, 0x1D56C))                # double-struck
    cps |= {0x210E, 0x2102, 0x210D, 0x2115, 0x2119, 0x211A, 0x211D, 0x2124,  # h, bb holes
            0x2212, 0x00B1, 0x2213, 0x00D7, 0x00F7, 0x22C5, 0x2219, 0x2218, 0x2207, 0x2202,
            0x221E, 0x2264, 0x2265, 0x2260, 0x2248, 0x2261, 0x223C, 0x2243, 0x221D, 0x2192,
            0x2190, 0x2194, 0x21D2, 0x21D0, 0x21D4, 0x21A6, 0x2208, 0x2209, 0x2282, 0x2286,
            0x222A, 0x2229, 0x2200, 0x2203, 0x00AC, 0x2227, 0x2228, 0x2211, 0x220F, 0x222B,
            0x222C, 0x222E, 0x221A, 0x2032, 0x2033, 0x22EF, 0x2026, 0x22EE, 0x22F1, 0x27E8,
            0x27E9, 0x2016, 0x230A, 0x230B, 0x2308, 0x2309, 0x02C6, 0x02DC, 0x02D9, 0x00A8,
            0x20D7, 0x2190, 0x00B7, 0x2032, 0x2205, 0x2113, 0x03D5, 0x03F5, 0x03D1, 0x03F0,
            0x03F1, 0x03D6, 0x1D716, 0x1D717, 0x1D718, 0x1D719, 0x1D71A, 0x1D71B, 0x2217}
    return sorted(cps)


VARIANTS = [0x28, 0x29, 0x5B, 0x5D, 0x7B, 0x7D, 0x7C, 0x2016, 0x27E8, 0x27E9, 0x230A, 0x230B,
            0x2308, 0x2309, 0x221A, 0x2211, 0x220F, 0x222B, 0x222C, 0x222E]


def main():
    font = TTFont(FONT)
    assert font["head"].unitsPerEm == 1000, "the core assumes 1000 units per em"
    cmap = font.getBestCmap()
    gs = font.getGlyphSet()
    math = font["MATH"].table
    italics = {}
    ic = math.MathGlyphInfo.MathItalicsCorrectionInfo
    for name, rec in zip(ic.Coverage.glyphs, ic.ItalicsCorrection):
        italics[name] = rec.Value

    def glyph_line(key, name):
        g = gs[name]
        bp = BoundsPen(gs)
        g.draw(bp)
        box = bp.bounds or (0, 0, 0, 0)
        pen = SVGPathPen(gs, ntos=lambda v: str(round(v)))
        g.draw(pen)
        path = pen.getCommands().replace(" ", ",") or "M0,0"
        x0, y0, x1, y1 = (round(v) for v in box)
        return f"g {key} {g.width} {x0} {y0} {x1} {y1} {italics.get(name, 0)} {path}"

    lines = [f"# STIX Two Math {font['name'].getDebugName(5)}, SIL OFL 1.1; written by tools/font/extract.py",
             f"# source sha256 {hashlib.sha256(FONT.read_bytes()).hexdigest()}"]
    for k in CONSTANTS:
        v = getattr(math.MathConstants, k)
        lines.append(f"k {k} {getattr(v, 'Value', v)}")
    written = set()
    for cp in codepoints():
        if cp in cmap:
            lines.append(glyph_line(f"{cp:X}", cmap[cp]))
            written.add(cmap[cp])
    mv = math.MathVariants
    cover = mv.VertGlyphCoverage.glyphs
    for cp in VARIANTS:
        name = cmap.get(cp)
        if name not in cover:
            continue
        rec = mv.VertGlyphConstruction[cover.index(name)]
        keys = []
        for r in rec.MathGlyphVariantRecord:
            vname = r.VariantGlyph
            key = f"{cp:X}" if vname == name else vname
            if vname not in written:
                lines.append(glyph_line(key, vname))
                written.add(vname)
            keys.append(key)
        lines.append(f"v {cp:X} " + " ".join(keys))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(lines)} lines, {OUT.stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
