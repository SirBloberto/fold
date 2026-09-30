# Fold

**Live vector graphics: every pattern is space, folded.**

Fold (`.fld`) is a picture format that is also a tiny, safe program. A `.fld` file loads like an image, but it computes itself from its inputs every frame. So it can animate, react to hover or progress, and be driven by data, while staying small and readable.

```
~fold v1 256x256
input progress: 0..1 = 0

let sun = circle(80 + progress * 20)
let w = 20 + sin(TIME * 2) * 6

draw rect(30, 8) |> at(110, 0) |> around(12) |> spin(TIME * 0.2) |> fill(#ffaa00)
draw sun |> glow(#ffaa00, w)
draw sun |> fill(#ffaa00)
```

A sun with twelve spinning rays and a breathing glow, whose size follows a `progress` value set by the host app. It is sharp at any resolution.

## Why Fold

- **Layers like SVG:** a file is a list of `draw`s, painted in order.
- **Pixels like a shader:** every shape is a distance function and every look is per-pixel maths. Glows, soft shadows, patterns and noise are one call each.
- **Folding space:** repeat, mirror and rotate by folding space rather than copying shapes. Twelve rays cost the same as one.
- **Safe to load from anyone:** no I/O, no scripts, bounded work. A file can only compute pixels from the inputs it declares.
- **Deterministic:** the same file and inputs give the same pixels on every machine.

Good for animated icons and loaders, UI that reacts to hover or progress, data-driven badges, generative art, and procedural textures in kilobytes.

## Documentation

| Document | For |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | The whole language, on one page |
| [docs/COOKBOOK.md](docs/COOKBOOK.md) | Recipes for common goals |
| [src/prelude.fld](src/prelude.fld) | The standard library, written in Fold |
| [docs/ENGINE.md](docs/ENGINE.md) | Implementers: determinism, limits, host API, conformance |
| [docs/PRINCIPLES.md](docs/PRINCIPLES.md) | The design rules every change is checked against |

## Status

**Early development.** The engine runs the language in SPEC.md, reloads files live and renders on all cores. Each file is compiled once, when it loads, into a flat list of maths steps: every error is reported at load time, and checks that can never fire are removed. The canvas is drawn in tiles, and each tile keeps only the steps that can affect it, so a shape costs nothing where it cannot be seen. Each step runs once per frame, row, column or pixel, depending on what it reads, and tiles that do not depend on `TIME` are drawn once. It also runs in the browser as WebAssembly, with a gallery and a live editor whose sliders drive each file's inputs. It differs from the spec in four ways: a custom shape's bounding box is the whole canvas, so `anchor` and `pin` are exact only for built-in shapes; the 1 MB source limit is not enforced; `set_eased` is not implemented; and the desktop viewer shows every input at its default value. [`examples/`](examples/) has scenes from a five-line eclipse to a black hole, neon signs, meshing gears, op art, a living contour map and a field of fourteen thousand dots.

## Running

Requires [Rust](https://rustup.rs).

```bash
cargo run --release -- examples/sun.fld
```

A window opens and redraws live as you edit and save the file. The console prints frame timing once a second. Press Escape to close.

To save a single frame as a PNG, at the header's size or at any width:

```bash
cargo run --release -- render examples/blackhole.fld blackhole.png --time 5
cargo run --release -- render examples/blackhole.fld wallpaper.png --time 5 --width 3840
```

### In the browser

[`web/`](web/) holds the WebAssembly player ([fold.js](web/fold.js)) and the gallery. Build the player, then serve the repository root with any static server and open `/web/`:

```bash
rustup target add wasm32-unknown-unknown
cargo web
python -m http.server
```

The player runs on one thread and renders each picture at the size of its place on the page.

`cargo test` runs the unit tests and the conformance suite in [`tests/conformance`](tests/conformance).
