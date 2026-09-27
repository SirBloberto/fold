# Fold engine requirements

For people implementing Fold. The language itself is defined in [SPEC.md](SPEC.md), and the standard library in [prelude.fld](../src/prelude.fld).

## 1. Loading
1. Read the header. Refuse unknown versions, naming both the file's version and the supported one.
2. Load the prelude, then the file. The prelude is Fold source shipped with the engine.
3. Lex, parse and check. **Every error is reported at load time**, with a line and column: syntax, unknown names, wrong arguments, types, redefinition, recursion and limits. A file that loads successfully must render without errors. Error messages name types as `num`, `vec2`, `rgba`, `shape` and `func`.
## 2. Rendering
- The host chooses the output size. Canvas units are scaled uniformly to fit, and `PX` is the size of one output pixel in units *(open: letterbox or crop when the aspect ratio differs)*.
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

## 3. Determinism
- All arithmetic is IEEE-754 binary32, with no fused or reordered operations ("fast-math").
- Native maths functions use the engine's own specified implementations, not the platform's.
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
```
- Hosts track time in at least 64-bit, and pass `TIME` wrapped to at most 2¹⁶ seconds so that 32-bit precision holds.
- A scene with no `TIME` dependency and no changed inputs does not need to be re-rendered.

## 6. Optimisation
Engines may optimise freely, provided the pixels are identical (§3):
- evaluate parts that don't depend on `POS` once per frame;
- skip tiles that interval arithmetic proves are constant, using shape bounds;
- prune shapes that cannot affect a tile;
- recognise common prelude transforms (`at`, `spin`, `around`) and handle them natively.

## 7. Conformance
The test suite is a set of `.fld` files, each with an expected PNG rendered by the reference engine at a fixed size, time and inputs. There is one test for every language feature and every load error. A back end conforms when it passes every test at its level (§3).

