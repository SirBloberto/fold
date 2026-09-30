import { Fold } from "./fold.js";

const WASM = "../target/wasm32-unknown-unknown/web/fold_web.wasm";
const EXAMPLES = "../examples/";
const FEATURE = "blackhole";
const PIECES = [
    ["blackhole", "Black hole", "Flat shapes, a lensed disk and drifting planets, sharp at any size."],
    ["neon", "Neon", "Every tube glows onto the wall. In SVG, each glow is a slow blur filter."],
    ["contours", "Contours", "A living landscape from noise. As video, this would be megabytes."],
    ["gears", "Gears", "Five meshing gears. Each is one tooth, folded around its centre."],
    ["opart", "Op art", "Under a kilobyte. Almost all of its work runs once per row and column."],
    ["ripples", "Ripples", "Fourteen thousand dots, for the cost of one."],
    ["forest", "Forest", "Four rows of repeated pines, swaying in drifting fog."],
    ["lighthouse", "Lighthouse", "A sweeping beam over folded waves."],
    ["mandala", "Mandala", "Seventeen layers, each one shape folded around the centre."],
    ["aurora", "Aurora", "Six inputs shape the storm. Open it and drag the sliders."],
];
const STILL = 6;
const LONGEST = 960;
const SLOW = 40;

const fold = await Fold.start(WASM);
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

function bytes(text) {
    return new TextEncoder().encode(text).length;
}

function kilobytes(text) {
    const size = bytes(text);
    return size < 1000 ? `${size} bytes` : `${(size / 1024).toFixed(1)} KB`;
}

function cover(element, aspect) {
    const { width, height } = element.getBoundingClientRect();
    return Math.min(Math.max(width, height * aspect) * devicePixelRatio, LONGEST);
}

function contain(element, aspect) {
    const { width, height } = element.getBoundingClientRect();
    return Math.min(Math.min(width, height * aspect) * devicePixelRatio, LONGEST);
}

function timed(draw) {
    const start = performance.now();
    draw();
    return performance.now() - start;
}

function load(name) {
    const picture = fold.load(sources.get(name));
    return { picture, aspect: picture.width / picture.height };
}

let playing = null;

function card([name, title, note]) {
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
    const { picture, aspect } = load(name);
    let painted = 0;
    let started = 0;
    let average = 0;

    const still = () => {
        const width = Math.round(cover(frame, aspect));
        if (width === 0 || width === painted) return;
        painted = width;
        picture.resize(width);
        picture.draw(canvas, STILL);
    };
    const play = (now) => {
        if (playing !== link) return;
        const spent = timed(() => picture.draw(canvas, STILL + (now - started) / 1000));
        average = average ? average * 0.9 + spent * 0.1 : spent;
        badge.textContent = `${average.toFixed(1)} ms a frame`;
        requestAnimationFrame(play);
    };
    link.addEventListener("pointerenter", () => {
        playing = link;
        started = performance.now();
        average = 0;
        requestAnimationFrame(play);
    });
    link.addEventListener("pointerleave", () => {
        if (playing === link) playing = null;
        picture.draw(canvas, STILL);
    });
    return still;
}

const hero = $("#hero");
const feature = load(FEATURE);
$("#hero-bytes").textContent = kilobytes(sources.get(FEATURE));
let heroWidth = 0;

function heroTick(now) {
    if (!document.body.classList.contains("editing") && playing === null) {
        const width = Math.round(cover(hero.parentElement, feature.aspect) * 0.9);
        if (width !== heroWidth) {
            heroWidth = width;
            feature.picture.resize(width);
        }
        feature.picture.draw(hero, now / 1000);
    }
    requestAnimationFrame(heroTick);
}
requestAnimationFrame(heroTick);

const stills = PIECES.filter(([name]) => name !== FEATURE).map(card);

async function paint() {
    for (const still of stills) {
        if (document.body.classList.contains("editing")) return;
        still();
        await new Promise((done) => setTimeout(done));
    }
}
new ResizeObserver(() => paint()).observe(grid);

let current = null;
let aspect = 16 / 9;
let origin = 0;
let average = 0;
let frames = 0;
let scale = 1;
let chosen = new Map();
let layout = "";

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
        chosen.set(input.name, next);
        if (input.kind === "num") output.textContent = show(next);
        current?.set(input.name, next);
    });
    label.append(name, field, output);
    return label;
}

function offer(picture) {
    const inputs = picture.inputs();
    for (const input of inputs) {
        const value = chosen.get(input.name);
        if (value === undefined) continue;
        try {
            picture.set(input.name, value);
        } catch {
            chosen.delete(input.name);
        }
    }
    const shape = JSON.stringify(inputs.map(({ name, kind, lo, hi }) => [name, kind, lo, hi]));
    if (shape === layout) return;
    layout = shape;
    controls.replaceChildren(...inputs.map(control));
}

function fit() {
    if (!current) return;
    current.resize(Math.max(64, contain(box, aspect) * scale));
    frames = 0;
    average = 0;
}

function compile() {
    try {
        const picture = fold.load(code.value);
        aspect = picture.width / picture.height;
        box.style.aspectRatio = `${picture.width} / ${picture.height}`;
        box.style.maxWidth = `calc((100vh - 240px) * ${aspect})`;
        picture.resize(Math.max(64, contain(box, aspect) * scale));
        offer(picture);
        current?.unload();
        current = picture;
        problem.textContent = "";
    } catch (error) {
        problem.textContent = error.message;
    }
}

function tick(now) {
    if (current && document.body.classList.contains("editing")) {
        const spent = timed(() => current.draw(view, (now - origin) / 1000));
        average = average ? average * 0.9 + spent * 0.1 : spent;
        frames += 1;
        if (frames > 20 && average > SLOW && scale > 0.35) {
            scale *= 0.8;
            fit();
        }
        const size = `${current.width} × ${current.height}`;
        stats.textContent = `${size} · ${bytes(code.value).toLocaleString()} bytes · ${average.toFixed(1)} ms a frame`;
    }
    requestAnimationFrame(tick);
}
requestAnimationFrame(tick);

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
    origin = performance.now();
    chosen = new Map();
    layout = "";
    scale = 1;
    compile();
    scrollTo(0, 0);
}

function close() {
    document.body.classList.remove("editing");
    nav.innerHTML = `<a href="#">Gallery</a>`;
    current?.unload();
    current = null;
    controls.replaceChildren();
    layout = "";
    paint();
}

function route() {
    const name = location.hash.slice(1);
    if (sources.has(name)) open(name);
    else close();
}
addEventListener("hashchange", route);
route();
