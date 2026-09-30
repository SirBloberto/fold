import { Fold } from "./fold.js";

let fold = null;
let picture = null;
let queue = Promise.resolve();

async function handle(data) {
    if (data.kind === "start") {
        fold = await Fold.start(data.module);
    } else if (data.kind === "load") {
        picture?.unload();
        picture = fold.load(data.source);
        picture.resize(data.width);
        picture.band(data.top, data.rows);
        for (const [name, value] of data.values) picture.set(name, value);
    } else if (data.kind === "set") {
        picture?.set(data.name, data.value);
    } else if (data.kind === "frame") {
        const bytes = picture.changes(data.time);
        postMessage({ version: data.version, bytes }, [bytes.buffer]);
    }
}

onmessage = ({ data }) => {
    queue = queue.then(() => handle(data)).catch((error) => console.error(error));
};
