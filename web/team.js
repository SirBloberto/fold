const TILE_ROWS = 16;
const FRESH = Date.now();

export class Team {
    constructor(module, count) {
        this.version = 0;
        this.bands = [];
        this.waiting = null;
        this.workers = Array.from({ length: count }, () => {
            const worker = new Worker(new URL(`./worker.js?${FRESH}`, import.meta.url), { type: "module" });
            worker.onmessage = ({ data }) => this.arrive(data);
            worker.postMessage({ kind: "start", module });
            return worker;
        });
    }

    show(source, width, height, values, view) {
        this.version += 1;
        this.waiting?.done(null);
        this.waiting = null;
        const share = Math.ceil(height / this.workers.length / TILE_ROWS) * TILE_ROWS;
        this.bands = [];
        this.workers.forEach((worker, index) => {
            const top = index * share;
            const rows = Math.min(share, height - top);
            if (rows <= 0) return;
            this.bands.push({ worker });
            worker.postMessage({ kind: "load", source, width, top, rows, values: [...values], view });
        });
    }

    set(name, value) {
        for (const { worker } of this.bands) worker.postMessage({ kind: "set", name, value });
    }

    zoom(view) {
        for (const { worker } of this.bands) worker.postMessage({ kind: "zoom", view });
    }

    frame(time) {
        return new Promise((done) => {
            const version = this.version;
            this.waiting = { version, left: this.bands.length, parts: [], done };
            for (const { worker } of this.bands) {
                worker.postMessage({ kind: "frame", time, version });
            }
        });
    }

    arrive(part) {
        const waiting = this.waiting;
        if (!waiting || part.version !== waiting.version) return;
        waiting.parts.push(part);
        waiting.left -= 1;
        if (waiting.left > 0) return;
        this.waiting = null;
        waiting.done(waiting.parts);
    }
}

export class Player {
    constructor(team) {
        this.team = team;
        this.stills = [];
        this.active = null;
        this.shown = null;
        this.busy = false;
        requestAnimationFrame((now) => this.tick(now));
    }

    tick(now) {
        requestAnimationFrame((later) => this.tick(later));
        if (this.busy) return;
        const job = this.stills.shift() ?? this.active;
        if (!job?.width) return;
        const time = job.still ?? job.start + (now - job.origin) / 1000;
        this.busy = true;
        this.render(job, time).finally(() => {
            this.busy = false;
        });
    }

    play(job) {
        job.origin = performance.now();
        job.start ??= 0;
        this.active = job;
    }

    stop(job) {
        if (this.active === job) this.active = null;
    }

    refresh(job) {
        if (this.shown === job) this.shown = null;
    }

    set(job, name, value) {
        job.values.set(name, value);
        if (this.shown === job) this.team.set(name, value);
    }

    zoom(job, view) {
        job.view = view;
        job.moved = true;
    }

    async render(job, time) {
        if (this.shown !== job) {
            this.team.show(job.source, job.width, job.height, job.values, job.view);
            this.shown = job;
            job.moved = false;
            job.image = new ImageData(job.width, job.height);
        } else if (job.moved) {
            this.team.zoom(job.view);
            job.moved = false;
        }
        const start = performance.now();
        const parts = await this.team.frame(time);
        if (!parts) return;
        const { canvas, image } = job;
        if (canvas.width !== job.width || canvas.height !== job.height) {
            canvas.width = job.width;
            canvas.height = job.height;
        }
        const areas = [];
        for (const { bytes } of parts) unpack(bytes, image, areas);
        const context = canvas.getContext("2d");
        let sent = 0;
        for (const { left, top, width, height } of merge(areas)) {
            context.putImageData(image, 0, 0, left, top, width, height);
            sent += width * height;
        }
        job.drawn?.(performance.now() - start, sent / (job.width * job.height));
    }
}

function unpack(bytes, image, areas) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let at = 0;
    while (at < bytes.length) {
        const [left, top, width, height] = [0, 4, 8, 12].map((offset) => view.getUint32(at + offset, true));
        at += 16;
        for (let row = 0; row < height; row += 1) {
            const length = width * 4;
            image.data.set(bytes.subarray(at, at + length), ((top + row) * image.width + left) * 4);
            at += length;
        }
        areas.push({ left, top, width, height });
    }
}

function merge(areas) {
    areas.sort((a, b) => a.top - b.top || a.left - b.left);
    const merged = [];
    for (const area of areas) {
        const last = merged.at(-1);
        if (last && last.top === area.top && last.height === area.height && last.left + last.width === area.left) {
            last.width += area.width;
        } else {
            merged.push({ ...area });
        }
    }
    return merged;
}
