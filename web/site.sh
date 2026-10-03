#!/bin/sh
set -eu

out="${1:-site}"
root="$(cd "$(dirname "$0")/.." && pwd)"
wasm="target/wasm32-unknown-unknown/web/fold_web.wasm"

rm -rf "$out"
mkdir -p "$out/web" "$out/examples" "$out/$(dirname "$wasm")"
cp "$root"/web/index.html "$root"/web/*.js "$out/web/"
cp "$root"/examples/*.fld "$out/examples/"
cp "$root/$wasm" "$out/$wasm"
cat > "$out/index.html" <<'EOF'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Fold</title>
<meta http-equiv="refresh" content="0; url=web/">
</head>
<body><a href="web/">Fold gallery</a></body>
</html>
EOF
