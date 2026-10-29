"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. Drive the tool here the way a person would, and assert what the page
shows with `check(condition, "what this shows")`. Assert the effect a person
would see (the text drawn, the file saved), not the markup that should cause it.

Expected values in these checks are worked out by hand from the brief (C4),
never copied from the tool's own output. Wait on a selector or a data-*
attribute the page sets, never on a fixed time: on CI the page may draw with
a CPU and run slowly.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())
EXAMPLES = json.loads((ROOT / "data" / "examples.json").read_text())


SHELF = [e for e in EXAMPLES if e.get("group") == "shelf"]


def student(page, check):
    """The Build mode a student opens to (journal page 5)."""
    check(page.inner_text("#intro p") == STRINGS["kid_intro"], "a student reads the student intro")
    check(page.inner_text("#message") == "", "no error message is shown")
    labels = page.eval_on_selector_all("#examples option", "os => os.map(o => o.textContent)")
    check(labels == [e["label"] for e in SHELF], "the shelf offers the sixth-grade formulas only")
    check(not page.is_visible("#formulas") and not page.is_visible("#sheet"), "no LaTeX or sheet is on a student's screen")
    check(not page.is_visible("[data-export='latex']"), "no LaTeX export for a student")
    # A = l x w: five tiles; + then 2 at the end makes seven
    check(page.locator("#tile-lines .tile").count() == 5, "A = l x w is five tiles")
    page.click("#keys button:text-is('+')")
    page.click("#keys button:text-is('2')")
    page.wait_for_selector("#tile-lines .tline >> nth=0")
    check(page.locator("#tile-lines .tile").count() == 7, "two keys add two tiles")
    eq = page.locator("#keys button:text-is('=')")
    check(eq.is_disabled(), "a second = does not fit, so its key is off")
    page.click("#keys button[aria-label=%s]" % json.dumps(STRINGS["key_back"]))
    page.click("#keys button[aria-label=%s]" % json.dumps(STRINGS["key_back"]))
    check(page.locator("#tile-lines .tile").count() == 5, "delete twice takes both away")

    # Tap h in the triangle: its card opens as the height, part 2
    page.select_option("#examples", "triangle")
    page.wait_for_selector("#formula use[data-c='2']")
    page.locator("#formula use[data-c='2']").first.click(force=True)
    page.wait_for_selector("#card .look .badge")
    check(page.inner_text("#card .look .badge") == "2", "a tap on h opens the card of part 2")
    check(page.locator("#card select").first.input_value() == "height", "the card says it is the height")
    # Readers can drag the number: h becomes scrubbable, still at 4
    page.locator("#card select").nth(2).select_option("drag")
    page.wait_for_selector("#formula [data-scrub='2']")
    check(page.locator("#formula [data-scrub='2']").get_attribute("aria-valuetext") == "4", "the height can be dragged, from 4")

    # Blocks: the sharing example has three steps; add a fourth
    page.select_option("#examples", "share")
    page.wait_for_selector("#blocks .cblock")
    check(page.locator("#blocks .cblock").count() == 3, "three steps are three step blocks")
    page.click("#add-step")
    page.wait_for_selector("#blocks .cblock >> nth=3")
    check(page.locator("#steps li").count() == 4, "a new step block is a fourth step to play")
    check(page.evaluate("document.documentElement.scrollWidth <= innerWidth"), "the blocks fit the page: no sideways scroll")

    # A student's shared link opens in Explore, with no editor
    page.click("[data-export='link']:not([data-value])")
    page.wait_for_selector("#copy-dialog[open]")
    link = page.input_value("#copy-text")
    page.click("#close")
    base = page.url.split("/index.html")[0]
    # a pasted link opens a fresh page; going there from this page would be
    # only a change of fragment
    page.goto("about:blank")
    page.goto(base + "/index.html" + link[len(STRINGS["site"]):])
    page.wait_for_selector("body[data-ready='true']", state="attached")
    page.wait_for_selector("#roles button.on")
    check(page.inner_text("#roles button.on") == STRINGS["role_explore"], "a shared link opens in Explore")
    check(not page.is_visible("#tile-lines") and not page.is_visible("#card"), "Explore shows the reader alone")
    check(page.inner_text("#prose-out") == "The 3 multiplies each part inside the bracket. Press Play.", "the link opens the shared document, not the first on the shelf")
    page.goto(base + "/index.html")
    page.wait_for_selector("body[data-ready='true']", state="attached")


def run(page, check, engine):
    check(page.inner_text("#title") == STRINGS["title"], "the title from data/strings.json is drawn")
    student(page, check)
    page.click("#roles button:text-is(%s)" % json.dumps(STRINGS["role_teach"]))
    page.wait_for_selector("#examples option >> nth=%d" % (len(EXAMPLES) - 1), state="attached")
    check(not page.is_visible("#formulas"), "the code stays hidden until its switch is on")
    page.check("#code")
    page.wait_for_selector("#formulas", state="visible")
    check(True, "the switch shows the LaTeX in the Teacher mode")
    page.select_option("#examples", "line")
    page.wait_for_selector("#formula [data-scrub='1']")
    check(page.inner_text("#intro p") == STRINGS["intro"], "the intro from data/strings.json is drawn")
    labels = page.eval_on_selector_all("#examples option", "os => os.map(o => o.textContent)")
    check(labels == [e["label"] for e in EXAMPLES], "every example is offered by name")
    heads = page.eval_on_selector_all("#examples optgroup", "gs => gs.map(g => g.label)")
    fields = []
    for e in EXAMPLES:
        head = STRINGS["field_" + e["group"]]
        if head not in fields:
            fields.append(head)
    check(heads == fields, "the Teacher menu heads each field once, in order %s" % heads)
    broken = []
    for e in EXAMPLES:
        page.select_option("#examples", e["id"])
        page.wait_for_function("t => document.querySelector('#doc-title').value === t", arg=e["doc"]["title"])
        if page.is_visible("#errors") or page.is_visible("#plot-error"):
            broken.append(e["id"])
    check(broken == [], "every example opens with no error and no plot error %s" % broken)
    # a long fraction of tiles scrolls in its box instead of pushing the keypad over the reader
    page.select_option("#examples", "secant")
    page.wait_for_selector("#tile-lines .tfrac")
    editor = page.locator(".editor").bounding_box()
    keys = page.locator("#keys").bounding_box()
    check(keys["x"] + keys["width"] <= editor["x"] + editor["width"], "the keypad stays inside its column beside a wide line of tiles")
    page.select_option("#examples", "line")
    page.wait_for_selector("#formula [data-scrub='1']")
    # y = m x + b with the slope shown as its value 1.5: y = 1 . 5 x + b
    check(page.locator("#formula svg use").count() == 8, "y = 1.5 x + b draws eight glyphs")

    # The slope starts at 1.5 and steps by 0.5, held to its limit of 2.
    scrub = page.locator("#formula [data-scrub='1']")
    check(scrub.get_attribute("aria-valuetext") == "1.5", "the slope starts at 1.5")
    scrub.focus()
    page.keyboard.press("ArrowRight")
    page.wait_for_selector("#formula [data-scrub='1'][aria-valuetext='2.0']")
    check(True, "one step right makes the slope 1.5 + 0.5 = 2.0")
    page.keyboard.press("ArrowRight")
    page.wait_for_selector("#formula [data-scrub='1'][aria-valuetext='2.0']")
    check(page.locator("#formula [data-scrub='1']").get_attribute("aria-valuetext") == "2.0", "the limit holds the slope at 2.0")

    # Mono draws every concept in ink: no concept colour is left.
    page.click("#views button:nth-child(3)")
    page.wait_for_selector("#views button:nth-child(3).on")
    svg = page.inner_html("#formula")
    check("#1D6FA3" not in svg and "#8A5A00" not in svg, "Mono uses no concept colour")
    check("stroke-dasharray" in svg, "Mono keeps the dashed pattern under the intercept")
    page.click("#views button:nth-child(1)")

    # LaTeX export wraps the slope in its own macro.
    page.click("[data-export='latex']")
    page.wait_for_selector("#copy-dialog[open]")
    check("\\fbslope{m}" in page.input_value("#copy-text"), "the LaTeX export wraps m in \\fbslope")
    page.click("#close")

    # The derivation plays through its four steps and stops on the last.
    page.select_option("#examples", "complex")
    page.wait_for_selector("#steps li:nth-child(4)")
    page.click("#play")
    page.wait_for_selector("#steps li:nth-child(4) button.on", timeout=30000)
    page.wait_for_selector("#play:text-is(%s)" % json.dumps(STRINGS["play"]), timeout=30000)
    check(True, "Play steps through to the fourth step and stops")

    # The link export opens the same document in the embed page.
    page.select_option("#examples", "line")
    page.click("[data-export='link']:not([data-value])")
    page.wait_for_selector("#copy-dialog[open]")
    link = page.input_value("#copy-text")
    check(link.startswith(STRINGS["site"] + "#f="), "the link points at the published site")
    base = page.url.split("/index.html")[0]
    page.goto(base + "/embed.html" + link[len(STRINGS["site"]):])
    page.wait_for_selector("body[data-ready='true']", state="attached")
    LINE = next(e for e in EXAMPLES if e["id"] == "line")
    check(page.inner_text("#title") == LINE["doc"]["title"], "the embed draws the linked document")
    check(page.locator("#formula [data-scrub='1']").count() == 1, "the embed keeps the scrubbable slope")
