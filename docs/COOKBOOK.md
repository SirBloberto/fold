# Fold cookbook

Short recipes for common goals. Every recipe is a complete file body: add a header such as `~fold v1 256x256` above it. The language is defined in [SPEC.md](SPEC.md).

## Shapes and looks

### A filled shape
```
draw circle(60) |> fill(#ffaa00)
```

### Rounded rectangle with an outline
The outline is drawn after the fill, so it paints over the fill's edge.
```
let card = rounded_rect(160, 100, 16)
draw card |> fill(#ffffff)
draw card |> outline(2) |> fill(#dddddd)
```

### Inside or outside stroke
`grow` with a negative amount shrinks a shape, so the stroke sits fully inside the edge. A positive amount puts it outside.
```
let badge = circle(50)
draw badge |> fill(#3366ff)
draw badge |> grow(-2) |> outline(4) |> fill(#ffffff)
```

### Glow
```
let orb = circle(30)
draw orb |> glow(#44ccff, 12)
draw orb |> fill(#ffffff)
```

### Inner glow
`invert` turns the shape inside out so the glow falls inwards, and `cover` keeps it inside the shape.
```
let panel = rounded_rect(160, 100, 16)
draw panel |> fill(#101820)
draw panel |> invert |> glow(#44ccff, 10) * cover(panel)
```

### Drop shadow
```
let card = rounded_rect(160, 100, 16)
draw card |> at(0, 6) |> soft(#00000066, 10)
draw card |> fill(#ffffff)
```

### Gradient fill
`POS.y / SIZE.y + 0.5` runs from 0 at the top of the canvas to 1 at the bottom.
```
draw circle(90) |> fill(mix(#ff5f6d, #ffc371, POS.y / SIZE.y + 0.5))
```

### Darker, lighter and transparent
```
let brand = #3366ff
draw circle(30) |> at(-70, 0) |> fill(shade(brand, -0.3))
draw circle(30) |> fill(brand)
draw circle(30) |> at(70, 0) |> fill(brand * 0.5)
```

### Blobs that melt together
```
let one = circle(40) |> at(sin(TIME) * 50, 0)
let two = circle(30) |> at(0, cos(TIME) * 40)
draw smooth_union(one, two, 20) |> fill(#ff66aa)
```

## Motion

### Spinner
```
draw rect(24, 6) |> at(60, 0) |> around(12) |> spin(TIME * 2) |> fill(#ffffff)
```

### Pulsing dot
```
draw circle(20 + sin(TIME * 4) * 4) |> fill(#ff3344)
```

### Rotating by degrees
```
draw rect(120, 12) |> spin(45 * DEG) |> fill(#222222)
```

### Mandala
```
let petal = segment(vec2(30, 0), vec2(85, 0)) |> grow(7)
let pair = union(petal |> spin(14 * DEG), petal |> spin(-14 * DEG))
draw pair |> around(8) |> spin(TIME * 0.2) |> fill(#ff66aa)
```

## Inputs

### Progress ring
The host sets `progress` from 0 to 1. `turned` measures how far round the ring each pixel is, starting at the top and going clockwise.
```
input progress: 0..1 = 0.25

let ring = circle(80) |> outline(12)
let turned = mod(angle(POS) + TAU / 4, TAU) / TAU
draw ring |> fill(#333333)
draw ring |> fill(#44ccff) * smoothstep(progress, progress - 0.002, turned)
```

### Hover button
The host sets `hover` to 1 when the pointer is over the button, with `set_eased` for a smooth change.
```
input hover: 0..1 = 0

let button = rounded_rect(160, 48, 12)
draw button |> glow(#3366ff, 4 + hover * 10)
draw button |> fill(shade(#3366ff, hover * 0.15))
```

### Theme colour from the host
```
input accent = #3366ff

draw rounded_rect(140, 40, 20) |> fill(accent)
```

## Layout

### Badge in a corner
```
let badge = circle(14)
draw badge |> pin(TOP_RIGHT, anchor(FRAME, TOP_RIGHT) + vec2(-8, 8)) |> fill(#ff3344)
```

### Label under an icon
```
let icon = rounded_rect(64, 64, 14)
let label = rounded_rect(80, 12, 6) |> pin(TOP, anchor(icon, BOTTOM) + vec2(0, 10))
draw icon |> fill(#3366ff)
draw label |> fill(#cccccc)
```

## Patterns

### Diagonal stripes
```
let stripe = smoothstep(-0.1, 0.1, sin((POS.x + POS.y) * 0.2 + TIME * 3))
draw FRAME |> fill(mix(#fdf6e3, #f3e2bf, stripe))
```

### Polka dots
```
draw circle(6) |> repeat(24, 24) |> fill(#222222)
```

### Moving noise
```
draw FRAME |> fill(mix(#0b1d3a, #4fc3f7, noise(POS / 20 + vec2(TIME, 0))))
```
