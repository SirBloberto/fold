# Fold v1

A Fold file (`.fld`) is a picture written as code: **layers drawn in order, like SVG, with every pixel computed, like a shader.** The same inputs always give the same pixels. A file cannot read, write or fetch anything.

```
~fold v1 256x256
input progress: 0..1 = 0

let sun = circle(80 + progress * 20)
let w = 20 + sin(TIME * 2) * 6

draw rect(30, 8) |> at(110, 0) |> around(12) |> spin(TIME * 0.2) |> fill(#ffaa00)
draw sun |> glow(#ffaa00, w)
draw sun |> fill(#ffaa00)
```

This page is the whole language. The standard library is written in Fold ([prelude.fld](../src/prelude.fld)). Engine requirements are in [ENGINE.md](ENGINE.md).

## 1. Files
- **Header:** the first line is `~fold v1 <width>x<height>`. The size is two whole numbers giving the canvas in **units**, not pixels. It fixes the aspect ratio and what one unit means. Engines refuse versions they don't support.
- **Text:** UTF-8. Only ASCII outside comments. Comments start with `//` and run to the end of the line.
- **Lines:** a newline ends a statement. A statement continues onto the next line if the line ends with an operator, `,`, `(` or `{`, or if the next line starts with `|>`.

## 2. Tokens
- **Names:** `[a-zA-Z_][a-zA-Z0-9_]*`. All-capital names belong to the engine and the prelude.
- **Keywords:** `input let func return draw`.
- **Reserved:** `if else for in match and or not true false import export as while loop`.
- **Numbers:** `80`, `0.5`. All numbers are 32-bit floats.
- **Colours:** `#rrggbb` or `#rrggbbaa`, in sRGB.
- **Symbols:** `+ - * / |> => = .. . , : ( ) { }`.

## 3. Values
| Type | Make | Read |
|---|---|---|
| num | `80`, `0.5` | |
| vec2 | `vec2(x, y)` | `.x` `.y` give nums. Any two of `x`/`y`, such as `.yx`, give a vec2. |
| rgba | `#ffaa00`, `rgba(r, g, b, a)` | `.r` `.g` `.b` `.a` give nums. Three letters, such as `.rgb` or `.bgr`, give an opaque rgba. Four, such as `.bgra`, give an rgba. |
| shape | `circle`, `rect`, `segment`, `shape(fn)` | `dist(sh, pt)`: negative inside, positive outside. `anchor(sh, anc)`: a point on its bounding box. |
| func | `func` names, inline `pt => expr` | Call with `fn(x)`. Inline functions capture the names around them. |

Colours are read and written as **sRGB, 0 to 1, with straight alpha**: `#ff8800` has `.r = 1` and `.a = 1`. Internally they are held as premultiplied linear light, so all blending is physically correct.

Types are inferred. A function is checked at each call with the types it is given, so `mix` works on nums, vec2s and rgbas alike. Every type error is reported when the file loads.

## 4. Statements
| Statement | Meaning |
|---|---|
| `input name: min..max = default` | A num the host may set, clamped to the range. `min`, `max` and `default` are constants, with `default` inside the range. |
| `input name = #rrggbb` | An rgba the host may set, such as a theme colour. The default is a constant. |
| `let name = expr` | Names a value. Each name is defined once, before it is used. |
| `func name(a, b) { … return expr }` | Defines a function. Its body may contain `let` and `return` only. It must not be recursive. |
| `draw expr` | Composites a colour over the canvas. Each `draw` paints over every `draw` above it in the file. |

`input`, `func` and `draw` appear only at the top level. A file may redefine a lowercase prelude name, such as `glow`. The new definition applies only to the file's own code: prelude functions always use the prelude's own names. Capital names can never be redefined.

## 5. Built-ins
| Name | Meaning |
|---|---|
| `POS` | This pixel's position in units. `(0, 0)` is the canvas **centre**. x grows right, **y grows down**. |
| `TIME` | Seconds, given by the host |
| `SIZE` | The canvas size from the header, as a vec2 |
| `PX` | The size of one screen pixel, in units. It changes with the output size: a `256x256` file shown at 512 pixels wide has `PX = 0.5`. The prelude uses it for crisp edges at any resolution. |

**Angles** are in radians. `TAU` is one full turn and `DEG` is one degree, so `spin(45 * DEG)` turns an eighth of a turn. Because y grows down, **positive angles turn clockwise** on screen.

**Anchors.** The prelude names nine points on any box, as relative positions from `(-1, -1)` to `(1, 1)`: `TOP_LEFT TOP TOP_RIGHT LEFT MID RIGHT BOTTOM_LEFT BOTTOM BOTTOM_RIGHT`. `FRAME` is the canvas as a shape. `anchor(sh, anc)` gives a point on a shape's box, and `pin(sh, anc, to)` moves a shape so that its anchor sits on a point. `at` moves a shape *by* an amount; `pin` moves it *to* a place:
```
draw badge |> pin(TOP_RIGHT, anchor(FRAME, TOP_RIGHT) + vec2(-8, 8))
draw dot   |> pin(LEFT, anchor(icon, RIGHT) + vec2(4, 0))
```

## 6. Expressions
Operators, from weakest binding to strongest:

| Operators | Meaning |
|---|---|
| `pt => e` | Inline function. The body runs as far right as possible. |
| `+ -` | Add, subtract |
| `* /` | Multiply, divide |
| `a \|> f(b)` | Pipe: exactly `f(a, b)`. `a \|> f` without brackets means `f(a)`. It binds tighter than arithmetic, so `sun \|> fill(#ffaa00) * 0.5` fades the result. |
| `-x` | Negate |
| `f(…)` `.x` | Call, field |

Arithmetic works on:
- num with num;
- vec2 with vec2: `+ - * /`, component by component;
- vec2 with num: `*` and `/`, in either order for `*`;
- rgba `+` rgba (adds light);
- rgba `*` num, in either order. This scales every channel including alpha, so it **fades** the colour. `#ffaa00 * 0.5` is half-transparent orange, not darker orange. Use `shade` to darken or lighten.

Every other combination is an error.

## 7. Evaluation
The file runs **once per pixel**, top to bottom. The canvas starts transparent. Each `draw` first makes its colour valid (alpha clamped to `0..1`, and no channel brighter than its alpha), then composites it on top: `canvas = col + canvas × (1 − col.a)`. After the last line, the canvas is converted to sRGB. Engines may evaluate the file any way they like, as long as the pixels are identical.

## 8. Native functions
Everything not listed here is defined in the prelude, in Fold.

| Group | Functions |
|---|---|
| Maths | `sin cos atan2 sqrt exp pow floor abs min max` |
| Values | `vec2(x, y)`, `rgba(r, g, b, a)` |
| Random | `hash(pt)`: a repeatable number in `0..1` for a position. Identical on every engine. |
| Shapes | `circle(r)`, `rect(w, h)`, `segment(from, to)`, all centred on the origin; `shape(fn)`, where `fn(pt)` returns the true distance to the edge, or less; `dist(sh, pt)` |
| Bounds | `anchor(sh, anc)`: the point at relative position `anc` on the shape's bounding box (`TOP_LEFT` is `(-1, -1)`, `MID` is `(0, 0)`). Exact for built-in shapes; never too small for others. |

## 9. Grammar
```
file      = header { statement }
statement = ( "input" NAME [ ":" expr ".." expr ] "=" expr
            | "let" NAME "=" expr
            | "func" NAME "(" [ NAME { "," NAME } ] ")" "{" { statement } "}"
            | "return" expr
            | "draw" expr ) NEWLINE
expr      = NAME "=>" expr | sum
sum       = product { ( "+" | "-" ) product }
product   = pipe { ( "*" | "/" ) pipe }
pipe      = unary { "|>" NAME [ "(" [ args ] ")" ] }
unary     = "-" unary | postfix
postfix   = primary { "(" [ args ] ")" | "." NAME }
primary   = NUM | RGBA | NAME | "(" expr ")"
args      = expr { "," expr }
```
