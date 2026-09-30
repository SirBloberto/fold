import { Fold } from "./fold.js";
import { Player, Team } from "./team.js";

const WASM = "../target/wasm32-unknown-unknown/web/fold_web.wasm";
const EXAMPLES = "../examples/";
const PIECES = [
    ["blackhole", "Black hole", "A black hole with a tilted disk, orbiting dust and two drifting planets."],
    ["neon", "Neon", "Neon signs on a brick wall. The OPEN sign flickers."],
    ["contours", "Contours", "A topographic map of slowly changing terrain, with a tide."],
    ["gears", "Gears", "Five meshing gears, drawn as a blueprint."],
    ["opart", "Op art", "A black and white checkerboard, warped by moving waves."],
    ["ripples", "Ripples", "A grid of dots, sized by two overlapping ripples."],
    ["forest", "Forest", "Rows of pine trees in drifting fog, under a full moon."],
    ["lighthouse", "Lighthouse", "A lighthouse beam sweeping over the sea at night."],
    ["mandala", "Mandala", "Rings of shapes turning around a centre."],
    ["aurora", "Aurora", "Northern lights over a lake. Its six inputs have sliders."],
];
const STILL = 6;
const LARGEST = 4096;

const module = await WebAssembly.compileStreaming(fetch(WASM));
const fold = await Fold.start(module);
const player = new Player(new Team(module, Math.min(navigator.hardwareConcurrency || 4, 8)));
const sources = new Map(await Promise.all(PIECES.map(async ([name]) => {
    const response = await fetch(`${EXAMPLES}${name}.fld`);
    return [name, await response.text()];
})));

const $ = (selector) => document.querySelector(selector);
const grid = $("#grid");
const code = $("#code");
const view = $("#view");
const box = $("#box");
const stats = $("#stats");
const problem = $("#problem");
const controls = $("#inputs");
const nav = $("#nav");
const editing = () => document.body.classList.contains("editing");

function bytes(text) {
    return new TextEncoder().encode(text).length;
}

function kilobytes(text) {
    const size = bytes(text);
    return size < 1000 ? `${size} bytes` : `${(size / 1024).toFixed(1)} KB`;
}

function shape(source) {
    const picture = fold.load(source);
    const aspect = picture.width / picture.height;
    picture.unload();
    return aspect;
}

function sized(job, width) {
    const picture = fold.load(job.source);
    picture.resize(Math.min(Math.max(1, Math.round(width)), LARGEST));
    const changed = picture.width !== job.width || picture.height !== job.height;
    job.width = picture.width;
    job.height = picture.height;
    picture.unload();
    if (changed) player.refresh(job);
    return changed;
}

function cover(element, aspect) {
    const { width, height } = element.getBoundingClientRect();
    return Math.max(width, height * aspect) * devicePixelRatio;
}

function contain(element, aspect) {
    const { width, height } = element.getBoundingClientRect();
    return Math.min(width, height * aspect) * devicePixelRatio;
}

function job(source, canvas, extra) {
    return { source, canvas, values: new Map(), width: 0, height: 0, ...extra };
}

const cards = PIECES.map(([name, title, note]) => {
    const link = document.createElement("a");
    link.className = "card";
    link.href = `#${name}`;
    link.innerHTML = `
        <div class="frame"><canvas></canvas><span class="play"></span></div>
        <div class="words">
            <div class="name">${title}<span>${kilobytes(sources.get(name))}</span></div>
            <p>${note}</p>
        </div>`;
    grid.append(link);
    const frame = link.querySelector(".frame");
    const canvas = link.querySelector("canvas");
    const badge = link.querySelector(".play");
    const source = sources.get(name);
    const aspect = shape(source);
    const still = job(source, canvas, { still: STILL });
    const moving = job(source, canvas, { start: STILL });
    let average = 0;
    moving.drawn = (spent) => {
        average = average ? average * 0.9 + spent * 0.1 : spent;
        badge.textContent = `${average.toFixed(1)} ms a frame`;
    };
    link.addEventListener("pointerenter", () => {
        average = 0;
        player.play(moving);
    });
    link.addEventListener("pointerleave", () => {
        player.stop(moving);
        player.stills.push(still);
    });
    return () => {
        const width = cover(frame, aspect);
        const changed = sized(still, width);
        sized(moving, width);
        if (changed) player.stills.push(still);
    };
});

function layout() {
    if (editing()) return;
    for (const card of cards) card();
}
new ResizeObserver(layout).observe(grid);

let current = null;
let aspect = 16 / 9;
let average = 0;
let sent = null;
let chosen = new Map();
let known = "";

function hex(channels) {
    return "#" + channels.slice(0, 3).map((c) => Math.round(c * 255).toString(16).padStart(2, "0")).join("");
}

function channels(text, alpha) {
    return [1, 3, 5].map((at) => parseInt(text.slice(at, at + 2), 16) / 255).concat(alpha);
}

function show(value) {
    return Number(value.toPrecision(3)).toString();
}

function control(input) {
    const label = document.createElement("label");
    const name = document.createElement("span");
    name.textContent = input.name;
    const field = document.createElement("input");
    const output = document.createElement("output");
    const value = chosen.get(input.name) ?? input.value;
    if (input.kind === "num") {
        field.type = "range";
        field.min = input.lo;
        field.max = input.hi;
        field.step = (input.hi - input.lo) / 1000 || 1;
        field.value = value;
        output.textContent = show(value);
    } else {
        field.type = "color";
        field.value = hex(value);
    }
    field.addEventListener("input", () => {
        const next = input.kind === "num" ? Number(field.value) : channels(field.value, input.value[3]);
        if (input.kind === "num") output.textContent = show(next);
        chosen.set(input.name, next);
        if (current) player.set(current, input.name, next);
    });
    label.append(name, field, output);
    return label;
}

function offer(picture) {
    const inputs = picture.inputs();
    for (const [name, value] of chosen) {
        try {
            picture.set(name, value);
        } catch {
            chosen.delete(name);
        }
    }
    const signature = JSON.stringify(inputs.map(({ name, kind, lo, hi }) => [name, kind, lo, hi]));
    if (signature === known) return;
    known = signature;
    controls.replaceChildren(...inputs.map(control));
}

function fit() {
    if (current) sized(current, contain(box, aspect));
}

function compile() {
    let picture;
    try {
        picture = fold.load(code.value);
    } catch (error) {
        problem.textContent = error.message;
        return;
    }
    problem.textContent = "";
    aspect = picture.width / picture.height;
    box.style.aspectRatio = `${picture.width} / ${picture.height}`;
    box.style.maxWidth = `calc((100vh - 240px) * ${aspect})`;
    offer(picture);
    picture.unload();
    const start = current ? current.start + (performance.now() - current.origin) / 1000 : 0;
    current = job(code.value, view, { start, values: new Map(chosen) });
    current.drawn = (spent, share) => {
        average = average ? average * 0.9 + spent * 0.1 : spent;
        sent = sent === null ? share : sent * 0.9 + share * 0.1;
        const parts = [
            `${current.width} × ${current.height}`,
            `${bytes(code.value).toLocaleString()} bytes`,
            `${average.toFixed(1)} ms a frame`,
            `${(sent * 100).toFixed(1)}% of pixels sent`,
        ];
        stats.textContent = parts.join(" · ");
    };
    fit();
    player.play(current);
}

let pending = 0;
code.addEventListener("input", () => {
    clearTimeout(pending);
    pending = setTimeout(compile, 150);
});
code.addEventListener("keydown", (event) => {
    if (event.key !== "Tab") return;
    event.preventDefault();
    code.setRangeText("    ", code.selectionStart, code.selectionEnd, "end");
    code.dispatchEvent(new Event("input"));
});
new ResizeObserver(fit).observe(box);

function open(name) {
    document.body.classList.add("editing");
    nav.innerHTML = `<a href="#">← Gallery</a>`;
    code.value = sources.get(name);
    code.scrollTop = 0;
    chosen = new Map();
    known = "";
    average = 0;
    sent = null;
    current = null;
    player.stills.length = 0;
    compile();
    scrollTo(0, 0);
}

function close() {
    document.body.classList.remove("editing");
    nav.textContent = "Hover to animate · click to edit";
    if (current) player.stop(current);
    current = null;
    controls.replaceChildren();
    known = "";
    layout();
}

function route() {
    const name = location.hash.slice(1);
    if (sources.has(name)) open(name);
    else close();
}
addEventListener("hashchange", route);
route();
