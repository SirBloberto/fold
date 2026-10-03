export class Fold {
    static async start(module) {
        const instance = await WebAssembly.instantiate(module, {});
        return new Fold(instance.exports);
    }

    constructor(wasm) {
        this.wasm = wasm;
    }

    bytes(at, length) {
        return new Uint8Array(this.wasm.memory.buffer, at, length);
    }

    send(text) {
        const encoded = new TextEncoder().encode(text);
        const at = this.wasm.inbox(encoded.length);
        this.bytes(at, encoded.length).set(encoded);
    }

    message() {
        const bytes = this.bytes(this.wasm.message(), this.wasm.message_length());
        return new TextDecoder().decode(bytes);
    }

    load(source) {
        this.send(source);
        const id = this.wasm.load();
        if (id < 0) throw new Error(this.message());
        return new Picture(this, id);
    }
}

export class Picture {
    constructor(fold, id) {
        this.fold = fold;
        this.id = id;
    }

    get width() {
        return this.fold.wasm.width(this.id);
    }

    get height() {
        return this.fold.wasm.height(this.id);
    }

    inputs() {
        if (this.fold.wasm.inputs(this.id) === 0) return [];
        return this.fold.message().split("\n").map((line) => {
            const [name, kind, ...numbers] = line.split("\t");
            const values = numbers.map(Number);
            return kind === "num"
                ? { name, kind, lo: values[0], hi: values[1], value: values[2] }
                : { name, kind, value: values };
        });
    }

    set(name, value) {
        this.fold.send(name);
        const failed = Array.isArray(value)
            ? this.fold.wasm.set_colour(this.id, ...value)
            : this.fold.wasm.set_num(this.id, value);
        if (failed < 0) throw new Error(this.fold.message());
    }

    resize(width) {
        this.fold.wasm.resize(this.id, Math.max(1, Math.round(width)));
    }

    zoom({ scale, x, y }) {
        this.fold.wasm.zoom(this.id, scale, x, y);
    }

    band(top, rows) {
        this.fold.wasm.band(this.id, top, rows);
    }

    changes(time) {
        const length = this.fold.wasm.changes(this.id, time);
        return this.fold.bytes(this.fold.wasm.packed(this.id), length).slice();
    }

    draw(canvas, time) {
        const { width, height } = this;
        if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
        }
        const at = this.fold.wasm.frame(this.id, time);
        const pixels = new Uint8ClampedArray(this.fold.wasm.memory.buffer, at, width * height * 4);
        canvas.getContext("2d").putImageData(new ImageData(pixels, width, height), 0, 0);
    }

    unload() {
        this.fold.wasm.unload(this.id);
    }
}
