import { Fold } from "./fold.js";
import { Player, Team } from "./team.js";

const WASM = "../target/wasm32-unknown-unknown/web/fold_web.wasm";
const EXAMPLES = "../examples/";
const PIECES = [
    ["blackhole", "Black hole", "A black hole that bends the starlight behind it, with two planets passing through."],
    ["glass", "Stained glass", "Irregular panes of coloured glass, lit by a sun that follows the mouse."],
    ["torch", "Torch", "A cellar lit by moonlight through a barred window and a torch that follows the mouse."],
    ["engraving", "Engraving", "Two spheres drawn only with ink lines, which thicken in shadow. The mouse moves the light."],
    ["kaleidoscope", "Kaleidoscope", "Coloured glass tumbling between three mirrors."],
    ["mercury", "Mercury", "Drops of liquid metal that merge and split. The mouse adds a drop of its own."],
    ["aurora", "Aurora", "Northern lights over a lake. Its six inputs have sliders."],
    ["opart", "Op art", "A black and white checkerboard, warped by moving waves."],
    ["snowflake", "Snowflake", "A Koch snowflake ten levels deep. Zoom in and the edges stay sharp."],
    ["valley", "Valley", "Hazy ridges in the afternoon, with a lookout tower, pines and birds."],
    ["neon", "Neon", "Neon signs on a brick wall. In the editor they can pulse with the microphone."],
    ["contours", "Contours", "A shaded relief map of slowly changing terrain, with a tide."],
    ["gears", "Gears", "Five meshing gears, drawn as a blueprint."],
    ["bubbles", "Bubbles", "Soap bubbles drifting upward, coloured by their swirling film."],
    ["ripples", "Ripples", "Dots sized by two overlapping ripples. In the editor it can listen to the microphone."],
    ["marble", "Marbling", "Bands of ink swirling like marbled paper."],
    ["forest", "Forest", "Rows of pine trees in drifting fog, under a full moon."],
    ["gauge", "Gauge", "Two instrument dials and a warning lamp, driven by three sliders."],
    ["mandala", "Mandala", "Turning rings of enamel set in gold wire."],
    ["truchet", "Truchet", "Glossy tubes on square tiles, turning one by one."],
];
const POINTER = ["pointer_x", "pointer_y"];
const SOUND = ["sound_level", "sound_bass", "sound_treble"];
const QUIET = { sound_level: 0.03, sound_bass: 0.12, sound_treble: 0.04 };
const STILL = 6;
const LARGEST = 4096;
const DEEPEST = 1000;
const WHOLE = { scale: 1, x: 0, y: 0 };

const module = await WebAssembly.compileStreaming(fetch(WASM, { cache: "no-cache" }));
const fold = await Fold.start(module);
const player = new Player(new Team(module, Math.min(navigator.hardwareConcurrency || 4, 8)));
const sources = new Map(await Promise.all(PIECES.map(async ([name]) => {
    const response = await fetch(`${EXAMPLES}${name}.fld`, { cache: "no-cache" });
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

function measure(picture) {
    const names = picture.inputs().map((input) => input.name);
    return {
        width: picture.width,
        height: picture.height,
        follows: POINTER.every((name) => names.includes(name)),
    };
}

function point(job, size, u, v) {
    player.set(job, POINTER[0], size.width * u);
    player.set(job, POINTER[1], size.height * v);
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
    return { source, canvas, values: new Map(), view: WHOLE, width: 0, height: 0, ...extra };
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
    const picture = fold.load(source);
    const size = measure(picture);
    picture.unload();
    const aspect = size.width / size.height;
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
    if (size.follows) {
        frame.addEventListener("pointermove", (event) => {
            const { left, top, width, height } = frame.getBoundingClientRect();
            const across = Math.max(width, height * aspect);
            const u = (event.clientX - left - width / 2) / across;
            const v = (event.clientY - top - height / 2) / (across / aspect);
            point(moving, size, u, v);
        });
    }
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
let header = { width: 1, height: 1, follows: false };
let lens = WHOLE;
let average = 0;
let sent = null;
let chosen = new Map();
let known = "";
let fields = new Map();
let hearing = null;

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
    fields.set(input.name, { field, output });
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
    fields = new Map();
    const shown = inputs.filter((input) => !POINTER.includes(input.name)).map(control);
    if (inputs.some((input) => SOUND.includes(input.name))) shown.unshift(microphone());
    controls.replaceChildren(...shown);
}

function microphone() {
    const button = document.createElement("button");
    button.textContent = hearing ? "Stop listening" : "Listen with the microphone";
    button.addEventListener("click", async () => {
        if (hearing) {
            deafen();
        } else {
            try {
                await listen();
            } catch (error) {
                problem.textContent = `The microphone is unavailable: ${error.message}`;
            }
        }
        button.textContent = hearing ? "Stop listening" : "Listen with the microphone";
    });
    return button;
}

async function listen() {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const audio = new AudioContext();
    const analyser = audio.createAnalyser();
    analyser.fftSize = 2048;
    analyser.smoothingTimeConstant = 0.7;
    audio.createMediaStreamSource(stream).connect(analyser);
    hearing = {
        stream,
        audio,
        analyser,
        bins: new Uint8Array(analyser.frequencyBinCount),
        wave: new Float32Array(analyser.fftSize),
        peaks: { ...QUIET },
    };
    requestAnimationFrame(hear);
}

function deafen() {
    if (!hearing) return;
    for (const track of hearing.stream.getTracks()) track.stop();
    hearing.audio.close();
    hearing = null;
}

function hear() {
    if (!hearing) return;
    requestAnimationFrame(hear);
    if (!current) return;
    const { audio, analyser, bins, wave, peaks } = hearing;
    analyser.getByteFrequencyData(bins);
    analyser.getFloatTimeDomainData(wave);
    const width = audio.sampleRate / analyser.fftSize;
    const band = (low, high) => {
        const from = Math.max(1, Math.floor(low / width));
        const to = Math.min(bins.length, Math.ceil(high / width));
        let sum = 0;
        for (let bin = from; bin < to; bin += 1) sum += bins[bin];
        return sum / Math.max(1, to - from) / 255;
    };
    const power = Math.sqrt(wave.reduce((sum, sample) => sum + sample * sample, 0) / wave.length);
    const raw = {
        sound_level: power,
        sound_bass: band(20, 250),
        sound_treble: band(2000, 8000),
    };
    for (const [name, loudness] of Object.entries(raw)) {
        peaks[name] = Math.max(loudness, peaks[name] * 0.998, QUIET[name]);
        const value = Math.min(loudness / peaks[name], 1);
        const shown = fields.get(name);
        if (!shown) continue;
        player.set(current, name, value);
        chosen.set(name, value);
        shown.field.value = value;
        shown.output.textContent = show(value);
    }
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
    header = measure(picture);
    aspect = picture.width / picture.height;
    box.style.aspectRatio = `${picture.width} / ${picture.height}`;
    box.style.maxWidth = `calc((100vh - 240px) * ${aspect})`;
    offer(picture);
    picture.unload();
    const start = current ? current.start + (performance.now() - current.origin) / 1000 : 0;
    current = job(code.value, view, { start, values: new Map(chosen), view: lens });
    current.drawn = (spent, share) => {
        average = average ? average * 0.9 + spent * 0.1 : spent;
        sent = sent === null ? share : sent * 0.9 + share * 0.1;
        const parts = [
            `${current.width} × ${current.height}`,
            `${bytes(code.value).toLocaleString()} bytes`,
            `${average.toFixed(1)} ms a frame`,
            `${(sent * 100).toFixed(1)}% of pixels sent`,
        ];
        if (lens.scale > 1) parts.push(`zoom ${show(lens.scale)}×`);
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

function steer({ scale, x, y }) {
    scale = Math.min(Math.max(scale, 1), DEEPEST);
    const inside = (at, full) => {
        const room = (full - full / scale) / 2;
        return Math.min(Math.max(at, -room), room);
    };
    lens = { scale, x: inside(x, header.width), y: inside(y, header.height) };
    if (current) player.zoom(current, lens);
}

function spot(event) {
    const { left, top, width, height } = view.getBoundingClientRect();
    return [(event.clientX - left) / width - 0.5, (event.clientY - top) / height - 0.5];
}

view.addEventListener("wheel", (event) => {
    event.preventDefault();
    const [u, v] = spot(event);
    const scale = Math.min(Math.max(lens.scale * Math.exp(-event.deltaY * 0.002), 1), DEEPEST);
    const across = header.width * u;
    const down = header.height * v;
    steer({
        scale,
        x: lens.x + across / lens.scale - across / scale,
        y: lens.y + down / lens.scale - down / scale,
    });
}, { passive: false });

let held = null;
view.addEventListener("pointerdown", (event) => {
    held = spot(event);
    view.setPointerCapture(event.pointerId);
    view.classList.add("held");
});
view.addEventListener("pointermove", (event) => {
    const [u, v] = spot(event);
    if (header.follows && current) {
        point(current, header, lens.x / header.width + u / lens.scale, lens.y / header.height + v / lens.scale);
        for (const name of POINTER) chosen.set(name, current.values.get(name));
    }
    if (!held) return;
    steer({
        ...lens,
        x: lens.x - (u - held[0]) * header.width / lens.scale,
        y: lens.y - (v - held[1]) * header.height / lens.scale,
    });
    held = [u, v];
});
const release = () => {
    held = null;
    view.classList.remove("held");
};
view.addEventListener("pointerup", release);
view.addEventListener("pointercancel", release);
view.addEventListener("dblclick", () => steer(WHOLE));

function open(name) {
    document.body.classList.add("editing");
    nav.innerHTML = `<span>Scroll to zoom · drag to move · double-click to reset</span><a href="#">← Gallery</a>`;
    code.value = sources.get(name);
    lens = WHOLE;
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
    deafen();
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
