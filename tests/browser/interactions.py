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


def run(page, check, engine):
    check(page.inner_text("#title") == STRINGS["title"], "the title from data/strings.json is drawn")
    check(page.inner_text("#intro p") == STRINGS["intro"], "the intro from data/strings.json is drawn")
    check(page.inner_text("#message") == "", "no error message is shown")
    labels = page.eval_on_selector_all("#examples option", "os => os.map(o => o.textContent)")
    check(labels == [e["label"] for e in EXAMPLES], "every example is offered by name")
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
    page.click("[data-export='link']")
    page.wait_for_selector("#copy-dialog[open]")
    link = page.input_value("#copy-text")
    check(link.startswith(STRINGS["site"] + "#f="), "the link points at the published site")
    base = page.url.split("/index.html")[0]
    page.goto(base + "/embed.html" + link[len(STRINGS["site"]):])
    page.wait_for_selector("body[data-ready='true']", state="attached")
    check(page.inner_text("#title") == EXAMPLES[0]["doc"]["title"], "the embed draws the linked document")
    check(page.locator("#formula [data-scrub='1']").count() == 1, "the embed keeps the scrubbable slope")
