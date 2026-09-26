# Fold design principles

Internal reference. Every change to the language, the prelude or the engine is checked against this page. If a change breaks a rule, either the change is wrong or the rule gets rewritten here first, deliberately.

## 1. What Fold is
**SVG's structure with a shader's engine.** A file describes a scene as layers drawn in order, and every pixel of it is computed. It is a *document* (it has a size and loads like an image), not a program for a surface.

## 2. Purity and safety
1. **A picture is a function of its inputs.** No state, no I/O, no clock, no true randomness. `TIME` and inputs come only from the host.
2. **A file cannot act, only answer.** Anything a host needs (hit regions, sizes, easing) is either computed from declared values or done by the engine, never by the file.
3. **Bounded work.** No recursion, loops only over sizes known at load time, limits on everything. A hostile file can never hang or crash its host.
4. **Deterministic.** The CPU reference is bit-exact. Other back ends stay within 1/255. Anything that would magnify tiny float differences (like `sin`-based hashing) is native and exact.

## 3. Size
1. **The spec fits on one page.** If a feature needs more than a few lines to explain, it's either too complicated or belongs in the prelude.
2. **Small native core.** A function is native only if it:
   - can't be written in Fold (`shape`, `dist`, bounds);
   - must be exact across hardware (`hash`); or
   - is proven too slow as prelude.

   Every native function costs an implementation in every back end, plus an interval version.
3. **Everything else is prelude,** written in Fold, readable by users.
4. **The prelude's list of functions fits on one screen.** A function earns its place only if nearly everyone needs it and it isn't one obvious line of what already exists.
5. **Compose before adding.** Pipes and composition come first (`sh |> outline(2) |> fill(#ffaa00)`), then a new function only if composition is genuinely awkward, or subtly wrong (`shade` exists because `mix` towards black changes alpha).

## 4. One way per thing
1. **No aliases, no duplicates.** If two functions do the same job, one goes. "Same job" means the same **intent**: moving *by* an amount (`at`) and moving *to* a point (`pin`) are different intents, so both stay.
2. **A name means one thing everywhere.** Anchors are always relative positions on a box, never sometimes positions and sometimes directions.
3. **One general tool beats many special cases.** `invert`, `cover` and negative `grow` cover every inside effect (inner glow, inner shadow, inside stroke), so there are no `inner_*` functions. `shade` covers darken and lighten. Anchors plus `pin` cover all layout.
4. **Signed values carry direction.** Negative means the other way (`grow(-2)` shrinks, `shade(-0.3)` darkens) rather than adding a second function.

## 5. Reading and writing
1. **Files read as sentences, top to bottom.** Pipes run in the order you'd describe the result. Each `draw` paints over every `draw` above it.
2. **The subject comes first.** Shape functions take the shape first, colour functions the colour first, so everything works with `|>`.
3. **Explicit over implicit.** Declared inputs, a required header, no silent type conversions, errors instead of guesses.
4. **Every error is found at load time,** with a line and column. A file that loads never fails while rendering.
5. **Users see what they wrote.** Colours are read and written in sRGB with straight alpha. Premultiplied linear light is an engine detail.

## 6. Naming
| Kind | Style | Examples |
|---|---|---|
| Keywords | short lowercase words | `let func draw input` |
| Types | short lowercase | `num vec2 rgba shape func` |
| Functions | lowercase `snake_case`, short but not cryptic | `spin around rounded_rect` |
| Engine and prelude constants | `CAPITALS` | `POS TIME SIZE PX TAU TOP_LEFT FRAME` |
| Parameters | short forms, never a single letter | `sh pt fn col amt vec anc by to` |
| Allowed single letters | conventional maths symbols only | `x y w h r n` |

- **Short forms when available:** `col`, `pos`, `amt`, `MID`, `PX`.
- **One name per thing.** The language accepts exactly one name for each function and constant.
- **No prelude aliases.** A prelude function whose body only calls another function, with its arguments renamed or reordered, is an alias and is not allowed. Every prelude function has a one-line intent, and no two intents are the same.
- **Transforms are verbs** (`spin`, `pin`, `grow`), **shapes are nouns** (`circle`, `rect`), **looks describe the result** (`fill`, `glow`, `soft`).
- **Capitals are reserved** for the engine and the prelude. Users can never redefine them, so new built-ins never collide with user names.

## 7. Coordinates and units
- The canvas centre is `(0, 0)`. **y grows down.**
- Sizes are **canvas units**, not pixels. The header's size is a coordinate system.
- **Angles are radians,** and positive angles turn **clockwise**.
- Shapes are **centred on their own origin**, so rotation and scaling happen around their middle.

## 8. The engine must be able to understand files
- **Shapes are values,** not bare numbers, so the engine knows their bounds. That keeps culling, layout, hit testing, export and debugging possible.
- **Custom is always allowed, never required.** Anything the engine can't analyse still renders correctly; it's just optimised less.
- **Efficiency means doing less, not only doing it in parallel.** Prefer designs that let the engine skip work: bounds, hoisting of per-frame values, unchanged frames.

## 9. Stability
- **Additions must not break existing files.** Future keywords are reserved now. New prelude names can be overridden by files that already use them.
- **The header grammar never changes.** Versions are whole numbers, bumped only when old engines would misread new files.
- **The CPU tree-walker is the reference.** Every optimisation and back end is checked against it, pixel for pixel, through the conformance tests.

## 10. Checklist for any new feature
1. Which gallery piece needs it? No real use means no feature.
2. Can it be composed from what exists? If so, don't add it.
3. Can it be prelude instead of native?
4. Does it keep one way per thing, and one meaning per name?
5. Does it read as a sentence in a pipe?
6. Can the engine still bound it, hoist it and check it at load time?
7. Is it deterministic and bounded?
8. Does the spec still fit on one page?
9. Does it break any existing file?
