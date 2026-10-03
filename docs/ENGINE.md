# Fold engine requirements

For people implementing Fold. The language itself is defined in [SPEC.md](SPEC.md), and the standard library in [prelude.fld](../src/prelude.fld).

## 1. Loading
1. Read the header. Refuse unknown versions, naming both the file's version and the supported one.
2. Load the prelude, then the file. The prelude is Fold source shipped with the engine.
3. Lex, parse and check. **Every error is reported at load time**, with a line and column: syntax, unknown names, wrong arguments, types, redefinition, recursion and limits. A file that loads successfully must render without errors. Error messages name types as `num`, `vec2`, `rgba`, `shape` and `func`.
## 2. Rendering
- The host chooses the output width. The height follows from the header's aspect ratio, rounded to whole pixels. `PX` is the size of one output pixel in canvas units: the header's width divided by the output width.
- The host may zoom in by a factor from 1 to 1000 around a centre point. The output then shows that part of the canvas at the same output size, `PX` shrinks by the zoom factor, and the centre is moved so the view stays inside the canvas.
- Each pixel is sampled at its centre.
- Colour literals are converted from sRGB to linear light when evaluated. All colour maths and compositing happen in premultiplied linear light, and the final canvas is converted back to sRGB.
- Each channel of the final canvas becomes an 8-bit sRGB value by counting how many of 255 thresholds it reaches. Threshold *k* is the linear-light value of sRGB (*k* − ½) / 255, rounded to the nearest 32-bit float. NaN and negative values give 0; values of 1 or more give 255.
- A colour containing NaN is drawn as transparent.

### 2.1 Bounds
Every shape carries a bounding box, which the engine uses for culling and which `anchor` reads.
- Built-in shapes have **exact** boxes.
- Recognised prelude transforms (`at`, `pin`, `spin`, `mirror`, `scale`, `grow`, `around`) compute their boxes directly from the inner box. A spun box is the box around the rotated corners.
- Other custom shapes are bounded by interval evaluation. The result may be larger than the shape, but never smaller. A shape with no finite bound covers the whole canvas.
- `repeat` has no finite bound.

Separately from its box, a shape may carry a **reach**: a box or a ring outside which its distance is never less than the distance to the reach. Engines use it only for culling, never for `anchor`. Built-in shapes reach their own box, and the prelude transforms derive their reach from their arguments:

| Transform | Reach |
|---|---|
| `at`, `pin` | the inner reach, moved |
| `spin`, `around` | a ring around the origin, from the nearest to the farthest point of the inner reach |
| `mirror` | the inner box and its reflection |
| `scale` by a constant above 0 | the inner reach, scaled |
| `union` | the box around both reaches |
| `smooth_union` with a constant blend above 0 | the box around both reaches, widened by a quarter of the blend |
| `intersect`, `subtract` | the first shape's reach |
| `grow`, `outline` | the inner reach, widened by the amount the distance is reduced |

Any other shape, including every `shape(fn)`, has no reach.

## 3. Determinism
- All arithmetic is IEEE-754 binary32, with no fused or reordered operations ("fast-math").
- Native maths functions use the engine's own specified implementations, not the platform's. [float.rs](../src/tape/float.rs) defines them exactly, including every constant and the order of operations: range reduction followed by a fixed polynomial, using only `+ − × ÷`, `floor` and bit operations. `sin` and `cos` are within 10⁻⁷ of the true value for |x| ≤ 8192, `exp` and `log` within 2 units in the last place, `atan2` within 4, and `pow` within 8 × max(1, \|y · ln x\|).
- `min(a, b)` is `b` when `b < a` or `a` is NaN, and `a` otherwise. `max(a, b)` is `b` when `b > a` or `a` is NaN, and `a` otherwise. So NaN is ignored, and of two equal values, including `-0` and `0`, the first is kept.
- `hash` uses exact integer arithmetic on the bits of its input, never floating-point tricks like `fract(sin(x) * 43758)`, which magnify tiny hardware differences into completely different values. Every back end, including the GPU, must return **bit-identical** results from `hash`.
- `hash(pt)` is defined as: add `0.0` to each component (so `-0.0` becomes `0.0`), take the IEEE-754 bits `x` and `y` as unsigned 32-bit integers, compute `s(x ^ s(y ^ 0x9e3779b9))`, and return the top 24 bits divided by 2²⁴. `s` is the lowbias32 mixer: `h ^= h >> 16; h *= 0x7feb352d; h ^= h >> 15; h *= 0x846ca68b; h ^= h >> 16`, with multiplication wrapping modulo 2³².
- **The CPU reference engine is bit-exact**: the same file, inputs, time and size give identical pixels everywhere.
- **Other back ends** (SIMD, GPU, WebAssembly) must match the reference to within **1/255 per channel**.

## 4. Limits
| Limit | Value |
|---|---|
| Source size | 1 MB |
| Nesting depth | 256 |
| `TIME` | 0 to 2²⁴ seconds (about 194 days). Hosts keep `TIME` in this range, since beyond it a 32-bit float can no longer tell one frame from the next. |
| Work per pixel | An instruction budget *(open: value)* |

A file that exceeds a limit fails to load.

## 5. Host interface
```
load(source)            → scene, or errors
scene.inputs()          → name, type, default and (for nums) range of each input
scene.set(name, value)  → nums are clamped to their range; rgbas are made valid
scene.set_eased(name, value, seconds)
                        → the engine animates the input to its new value
scene.render(time, width, height) → premultiplied sRGB pixels
scene.changed()         → the rectangles whose pixels differ from the previous render
```
- After the first render, every pixel outside `changed()` is identical to the previous render, so a host can copy or upload only those rectangles.
- A num set to NaN takes the bottom of its range.
- Hosts give and receive rgba inputs as sRGB with straight alpha, each channel from 0 to 1. The engine converts them to premultiplied linear light, as it does colour literals.
- Hosts track time in at least 64-bit, and pass `TIME` wrapped to at most 2¹⁶ seconds so that 32-bit precision holds.
- A scene with no `TIME` dependency and no changed inputs does not need to be re-rendered.

## 6. Optimisation
Engines may optimise freely, provided the pixels are identical (§3):
- evaluate each part once for everything it depends on: once per frame if it doesn't read `POS`, once per column if it reads only `POS.x`, and once per row if it reads only `POS.y`;
- skip tiles that interval arithmetic proves are constant, using shape bounds;
- prune shapes that cannot affect a tile, using their reach (§2.1);
- keep tiles that don't depend on `TIME` from one frame to the next, until an input changes;
- treat inputs as constants until the host first changes one, then as values within their declared ranges;
- recognise common prelude transforms (`at`, `spin`, `around`) and handle them natively.

## 7. Conformance
The test suite is the folder `tests/conformance`. It holds one small `.fld` file for every language feature and every load error, each with a reference beside it:
- a file that loads has a `.png` of the reference engine's pixels, rendered at the header's size with every input at its default;
- a file that fails to load has a `.txt` holding the exact error message.

A file renders at `TIME` 0 unless a line `// time <seconds>` gives another time. A back end conforms when it passes every test at its level (§3): the reference engine matches every PNG exactly, and other back ends match it to within 1/255 per channel.

The reference engine's runner is `cargo test --test conformance`. When a change is meant to alter pixels or messages, `FOLD_BLESS=1 cargo test --test conformance` rewrites the references, which are then checked by eye before they are committed.

