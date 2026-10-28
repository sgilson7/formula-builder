// The page draws what the core returns and decides nothing.
//
// The core sends the formula and the plot as finished SVG, the prose, the
// concept list, the controls and the steps; this file puts them on screen and
// sends every reader action back through `act`, the core's one door. Words
// come from data/strings.json through the core. (After Peter's Mandible.)
import init, { start, act, gif } from "./pkg/tool_shim.js";

const el = (id) => document.getElementById(id);
const make = (tag, className, text) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};
const mode = document.body.dataset.mode;

let state = null;
let glossary = "";
let ui = null;
let pending = null;
let looping = false;
let lastHover = "";
let scrubbing = false;
let moved = false;
let focusedScrub = "";
let controlShape = "";

function call(name, value) {
  return JSON.parse(act(JSON.stringify(state), glossary, name, value === undefined ? "" : value));
}

function send(name, value) {
  draw(call(name, value));
}

// Typing sends at most once a frame.
function queue(name, value) {
  const first = pending === null;
  pending = { name, value };
  if (first) {
    requestAnimationFrame(() => {
      const p = pending;
      pending = null;
      send(p.name, p.value);
    });
  }
}

function words() {
  document.querySelectorAll("[data-s]").forEach((node) => {
    const text = ui.strings[node.dataset.s];
    if (text !== undefined && node.textContent !== text) node.textContent = text;
  });
  (ui.hints || []).forEach((h) => {
    document.querySelectorAll(".head, summary, .small").forEach((node) => {
      if (node.textContent === h.term) {
        node.dataset.term = h.term;
        node.title = h.definition;
      }
    });
  });
}

function setText(id, value) {
  const node = el(id);
  if (node && document.activeElement !== node && node.value !== value) node.value = value;
}

function badge(num, colour) {
  const b = make("span", "badge", String(num));
  b.style.background = colour;
  return b;
}

function drawViews() {
  const box = el("views");
  box.replaceChildren(
    ...ui.views.map((v) => {
      const b = make("button", v.on ? "on" : "", v.label);
      b.type = "button";
      b.setAttribute("aria-pressed", v.on ? "true" : "false");
      b.addEventListener("click", () => send("view.set", v.id));
      return b;
    })
  );
}

function drawProse() {
  const p = el("prose-out");
  p.replaceChildren(
    ...ui.prose.map((seg) => {
      if (!seg.num) return document.createTextNode(seg.text);
      const s = make("span", seg.hovered ? "link on" : "link", seg.text);
      s.dataset.c = String(seg.num);
      s.style.color = seg.colour;
      s.style.textDecorationColor = seg.colour;
      s.style.textDecorationStyle = seg.line;
      return s;
    })
  );
}

function rangeInput(c, value, onInput) {
  const r = make("input");
  r.type = "range";
  r.min = c.min;
  r.max = c.max;
  r.step = c.step;
  r.value = value;
  r.style.accentColor = c.colour;
  r.setAttribute("aria-label", c.label);
  r.addEventListener("input", () => onInput(r.value));
  return r;
}

function drawControls() {
  const shape = ui.controls.map((c) => c.kind + c.name + c.min + c.max).join("|");
  const box = el("controls");
  if (shape === controlShape) {
    ui.controls.forEach((c, i) => {
      const row = box.children[i];
      if (!row) return;
      const inputs = row.querySelectorAll("input");
      const out = row.querySelector("output");
      if (c.kind === "limit") {
        if (document.activeElement !== inputs[0]) inputs[0].value = c.lo;
        if (document.activeElement !== inputs[1]) inputs[1].value = c.hi;
        out.textContent = c.lo + " – " + c.hi;
      } else {
        if (document.activeElement !== inputs[0]) inputs[0].value = c.value;
        out.textContent = c.value;
      }
    });
    return;
  }
  controlShape = shape;
  box.replaceChildren(
    ...ui.controls.map((c) => {
      const row = make("div", "control " + c.kind);
      const label = make("span", "control-label");
      if (c.num) label.append(badge(c.num, c.colour));
      label.append(make("span", "", c.label));
      row.append(label);
      if (c.num) row.dataset.c = String(c.num);
      if (c.kind === "limit") {
        row.append(rangeInput(c, c.lo, (v) => send("limit.set", "lo|" + c.name + "|" + v)));
        row.append(rangeInput(c, c.hi, (v) => send("limit.set", "hi|" + c.name + "|" + v)));
        row.append(make("output", "", c.lo + " – " + c.hi));
      } else {
        row.append(rangeInput(c, c.value, (v) => send("value.set", c.name + "|" + v)));
        row.append(make("output", "", c.value));
      }
      return row;
    })
  );
}

function drawSteps() {
  const bar = el("stepbar");
  bar.hidden = ui.steps.length === 0;
  if (bar.hidden) return;
  el("play").textContent = ui.playing ? ui.strings.pause : ui.strings.play;
  el("steps").replaceChildren(
    ...ui.steps.map((s, i) => {
      const li = make("li");
      const b = make("button", s.on ? "on" : "", s.label);
      b.type = "button";
      b.setAttribute("aria-current", s.on ? "step" : "false");
      b.addEventListener("click", () => send("step.go", String(i)));
      li.append(b);
      return li;
    })
  );
}

function drawConcepts() {
  if (!el("concept-list")) return;
  el("budget").textContent = ui.budget;
  const meter = el("meter");
  const cells = [];
  for (let i = 0; i < ui.budget_max; i++) {
    const cell = make("span", i < ui.budget_used ? "cell on" : "cell");
    cells.push(cell);
  }
  meter.replaceChildren(...cells);
  el("concept-list").replaceChildren(
    ...ui.concepts.map((c) => {
      const li = make("li", c.hovered ? "concept on" : "concept");
      li.dataset.c = String(c.num);
      const top = make("div", "concept-top");
      top.append(badge(c.num, c.colour));
      const sample = make("span", "sample");
      sample.innerHTML = c.sample;
      top.append(sample);
      top.append(make("strong", "", c.name));
      const eye = make("button", c.muted ? "eye off" : "eye", c.muted ? ui.strings.show : ui.strings.hide);
      eye.type = "button";
      eye.setAttribute("aria-pressed", c.muted ? "false" : "true");
      eye.addEventListener("click", () => send("concept.mute", String(c.num)));
      top.append(eye);
      const remove = make("button", "remove", "×");
      remove.type = "button";
      remove.title = ui.strings.remove;
      remove.setAttribute("aria-label", ui.strings.remove + " " + c.name);
      remove.addEventListener("click", () => send("concept.remove", String(c.num)));
      top.append(remove);
      li.append(top);
      if (c.say) li.append(make("p", "say", c.say));
      const toggles = make("div", "toggles");
      c.features.forEach((f) => {
        const t = make("button", f.on ? "toggle on" : "toggle", f.label);
        t.type = "button";
        t.setAttribute("aria-pressed", f.on ? "true" : "false");
        t.addEventListener("click", () => send("concept.toggle", c.num + " " + f.id));
        toggles.append(t);
      });
      li.append(toggles);
      return li;
    })
  );
  el("selection").textContent = ui.selection;
  el("add").disabled = ui.selection === "";
  el("no-selection").hidden = ui.selection !== "";
}

function drawErrors() {
  const box = el("errors");
  box.hidden = ui.errors.length === 0;
  box.querySelector("ul").replaceChildren(...ui.errors.map((e) => make("li", "", e)));
}

function draw(result) {
  if (result.error) {
    el("message").textContent = result.error;
    return;
  }
  ui = result.ok;
  state = ui.state;
  el("message").textContent = "";
  document.title = mode === "embed" ? ui.doc.title : ui.strings.title + " · " + ui.doc.title;
  el("title").textContent = mode === "embed" ? ui.doc.title : ui.strings.title;
  words();
  if (el("intro")) el("intro").querySelector("p").textContent = ui.strings.intro;
  if (el("formula").dataset.svg !== ui.formula_svg) {
    el("formula").innerHTML = ui.formula_svg;
    el("formula").dataset.svg = ui.formula_svg;
    if (focusedScrub) {
      const again = el("formula").querySelector("[data-scrub='" + focusedScrub + "']");
      if (again) again.focus();
    }
  }
  if (el("plot").dataset.svg !== ui.plot_svg) {
    el("plot").innerHTML = ui.plot_svg;
    el("plot").dataset.svg = ui.plot_svg;
  }
  el("plot-error").hidden = ui.plot_error === "";
  el("plot-error").textContent = ui.plot_error;
  drawViews();
  drawProse();
  drawControls();
  drawSteps();
  drawErrors();
  if (mode === "edit") {
    setText("doc-title", ui.doc.title);
    setText("formulas", ui.doc.formulas);
    setText("sheet", ui.doc.sheet);
    setText("prose", ui.doc.prose);
    const select = el("examples");
    if (select.options.length !== ui.examples.length) {
      select.replaceChildren(...ui.examples.map((e) => {
        const o = make("option", "", e.label);
        o.value = e.id;
        return o;
      }));
    }
    const current = ui.examples.find((e) => e.on);
    if (current) select.value = current.id;
    drawConcepts();
    el("clear").checked = ui.export_clear;
    // The address carries the document; Safari refuses frequent updates, so
    // write it only when the document changed.
    if (location.hash !== ui.fragment) history.replaceState(null, "", ui.fragment);
  } else {
    el("open-full").href = "./" + ui.fragment;
  }
  if (ui.out) deliver(ui.out);
  if (ui.playing && !looping) {
    looping = true;
    requestAnimationFrame(tick);
  }
}

function tick(now) {
  if (!ui.playing) {
    looping = false;
    return;
  }
  send("tick", String(now));
  requestAnimationFrame(tick);
}

// ---- files and exports: the core makes them; the page saves them

function save(blob, name) {
  const a = make("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(a.href);
}

function rasterise(svgText, width, height) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const canvas = make("canvas");
      canvas.width = width;
      canvas.height = height;
      const g = canvas.getContext("2d");
      g.drawImage(img, 0, 0, width, height);
      resolve(canvas);
    };
    img.onerror = reject;
    img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svgText);
  });
}

async function deliver(out) {
  if (out.kind === "download") {
    save(new Blob([out.text], { type: out.mime }), out.name);
  } else if (out.kind === "copy") {
    el("copy-text").value = out.text;
    el("copy").textContent = ui.strings.copy;
    el("copy-dialog").showModal();
    el("copy-text").select();
  } else if (out.kind === "png") {
    const canvas = await rasterise(out.text, out.width, out.height);
    canvas.toBlob((blob) => save(blob, out.name), out.mime);
  } else if (out.kind === "gif") {
    el("message").textContent = ui.strings.export_drawing;
    const pictures = [];
    for (const f of out.frames) {
      const canvas = await rasterise(f.svg, out.width, out.height);
      pictures.push(canvas.getContext("2d").getImageData(0, 0, out.width, out.height).data);
    }
    const size = pictures[0].length;
    const all = new Uint8Array(size * pictures.length);
    pictures.forEach((p, i) => all.set(p, i * size));
    const bytes = gif(all, out.width, out.height, out.frames.map((f) => f.delay).join(","));
    save(new Blob([bytes], { type: out.mime }), out.name);
    el("message").textContent = "";
  }
}

// ---- reader events: hover, select, scrub, edit

function wireReader() {
  const reader = document.querySelector(".reader");
  const hoverFrom = (e) => {
    const c = e.target.closest("[data-c]");
    const value = c ? c.dataset.c : "";
    if (value !== lastHover && !scrubbing) {
      lastHover = value;
      send("hover", value);
    }
  };
  reader.addEventListener("pointerover", hoverFrom);
  reader.addEventListener("pointerleave", () => {
    if (lastHover !== "") {
      lastHover = "";
      send("hover", "");
    }
  });
  const box = el("formula");
  box.addEventListener("pointerdown", (e) => {
    const s = e.target.closest("[data-scrub]");
    moved = false;
    if (!s) return;
    scrubbing = true;
    box.setPointerCapture(e.pointerId);
    send("scrub.start", s.dataset.scrub + " " + String(e.clientX));
    e.preventDefault();
  });
  box.addEventListener("pointermove", (e) => {
    if (!scrubbing) return;
    moved = true;
    queue("scrub.move", String(e.clientX));
  });
  const end = () => {
    if (!scrubbing) return;
    scrubbing = false;
    send("scrub.end");
  };
  box.addEventListener("pointerup", end);
  box.addEventListener("pointercancel", end);
  box.addEventListener("click", (e) => {
    if (mode !== "edit" || moved) return;
    const n = e.target.closest("[data-n]");
    if (n) send("select", n.dataset.n);
  });
  box.addEventListener("keydown", (e) => {
    const s = e.target.closest("[data-scrub]");
    if (!s) return;
    const dir = { ArrowRight: "up", ArrowUp: "up", ArrowLeft: "down", ArrowDown: "down" }[e.key];
    if (!dir) return;
    e.preventDefault();
    focusedScrub = s.dataset.scrub;
    send("scrub.key", s.dataset.scrub + " " + dir);
  });
  box.addEventListener("focusout", () => {
    focusedScrub = "";
  });
  box.addEventListener("change", (e) => {
    const input = e.target.closest("[data-edit]");
    if (input) send("value.set", input.dataset.name + "|" + input.value);
  });
  el("play").addEventListener("click", () => send("play"));
  el("prev").addEventListener("click", () => send("step.prev"));
  el("next").addEventListener("click", () => send("step.next"));
}

function wireEditor() {
  [["doc-title", "doc.title"], ["formulas", "doc.formulas"], ["sheet", "doc.sheet"], ["prose", "doc.prose"]].forEach(([id, name]) => {
    el(id).addEventListener("input", () => queue(name, el(id).value));
  });
  el("examples").addEventListener("change", () => send("doc.example", el("examples").value));
  el("new").addEventListener("click", () => send("doc.new"));
  el("save").addEventListener("click", () => send("export.json"));
  el("open").addEventListener("change", async () => {
    const file = el("open").files[0];
    if (file) send("doc.open", await file.text());
    el("open").value = "";
  });
  el("size").addEventListener("change", () => send("export.size", el("size").value));
  el("clear").addEventListener("change", () => send("export.clear"));
  document.querySelectorAll("[data-export]").forEach((b) => {
    b.addEventListener("click", () => send("export." + b.dataset.export));
  });
  el("add").addEventListener("click", () => send("concept.add"));
  const list = el("concept-list");
  list.addEventListener("pointerover", (e) => {
    const c = e.target.closest("[data-c]");
    const value = c ? c.dataset.c : "";
    if (value !== lastHover) {
      lastHover = value;
      send("hover", value);
    }
  });
  el("copy").addEventListener("click", async () => {
    el("copy-text").select();
    try {
      await navigator.clipboard.writeText(el("copy-text").value);
      el("copy").textContent = ui.strings.copied;
    } catch (err) {
      el("copy").textContent = ui.strings.copy;
    }
  });
  el("close").addEventListener("click", () => el("copy-dialog").close());
}

async function main() {
  await init();
  const strings = await (await fetch("data/strings.json")).text();
  const examples = await (await fetch("data/examples.json")).text();
  glossary = await (await fetch("data/glossary.md")).text();
  draw(JSON.parse(start(strings, examples, glossary, mode)));
  if (location.hash) send("doc.fragment", location.hash);
  wireReader();
  if (mode === "edit") wireEditor();
  document.body.dataset.ready = "true";
}

main();
