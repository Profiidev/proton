#!/usr/bin/env bash
# Captures fixtures/reference/<slug>.png for every route of the Tauri app (Hyprland only).
# Usage: fixtures/scripts/reference.sh [slug-filter]   e.g. `just reference settings`
# Env: PROTON_BIN (default target/release/proton), WAIT (seconds after the window shows, default 11: the offline toast lasts 10s)
set -euo pipefail
[[ -n ${TRACE:-} ]] && set -x

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${PROTON_BIN:-$ROOT/target/release/proton}
OUT=$ROOT/fixtures/reference
WAIT=${WAIT:-11}
FILTER=${1:-}

if [[ ! -x $BIN ]]; then
  (cd "$ROOT/app" && npx tauri build --no-bundle)
fi
grim() { if type -P grim >/dev/null; then command grim "$@"; else nix run nixpkgs#grim -- "$@"; fi; }
# Hyprland >= 0.56 (lua config) takes lua dispatchers, older versions the classic syntax
disp() { hyprctl dispatch "$1" | grep -q '^ok' || hyprctl dispatch $2 >/dev/null || true; }
mkdir -p "$OUT"

# shellcheck source=routes.sh
source "$ROOT/fixtures/scripts/routes.sh"

for route in "${ROUTES[@]}"; do
  IFS='|' read -r slug path mode <<<"$route"
  [[ -n $FILTER && $slug != *$FILTER* ]] && continue

  tmp=$(mktemp -d)
  cp -r "$ROOT/fixtures/data/." "$tmp/"
  jq --arg url "tauri://localhost$path" '.settings.url = $url' \
    "$tmp/io.profidev.proton/store.json" >"$tmp/store.json"
  mv "$tmp/store.json" "$tmp/io.profidev.proton/store.json"

  net=()
  [[ $mode == offline ]] && net=(unshare -n --map-current-user)
  # setsid: own process group (pgid = $pid) to find the window and kill everything after.
  # dbus-run-session: private session bus, so the single-instance plugin ignores a running Proton.
  XDG_DATA_HOME=$tmp XDG_CONFIG_HOME=$tmp/config XDG_CACHE_HOME=$tmp/cache \
    setsid dbus-run-session -- "${net[@]}" "$BIN" >"$tmp/stdout.log" 2>&1 &
  pid=$!

  client=""
  for _ in $(seq 100); do
    while read -r c; do
      [[ $(ps -o pgid= -p "$(jq .pid <<<"$c")" | tr -d ' ') == "$pid" ]] && client=$c
    done < <(hyprctl clients -j | jq -c '.[] | select(.class == "proton")')
    [[ -n $client ]] && break
    sleep 0.2
  done
  if [[ -z $client ]]; then
    echo "FAIL $slug: no window (log: $tmp/stdout.log)"
    kill -- -"$pid" 2>/dev/null || true
    continue
  fi

  addr=$(jq -r .address <<<"$client")
  w="window = \"address:$addr\""
  disp "hl.dsp.window.float({ action = \"enable\", $w })" "setfloating address:$addr"
  disp "hl.dsp.window.resize({ x = 1300, y = 800, $w })" "resizewindowpixel exact 1300 800,address:$addr"
  disp "hl.dsp.window.move({ x = 40, y = 40, $w })" "movewindowpixel exact 40 40,address:$addr"
  delay=$WAIT
  [[ $mode == online ]] && delay=$((WAIT + 6))
  sleep "$delay"

  client=$(hyprctl clients -j | jq -c --arg a "$addr" '.[] | select(.address == $a)')
  # hyprland may report 1301 wide, always grab exactly 1300x800 from the window origin
  geo=$(jq -r '"\(.at[0]),\(.at[1]) 1300x800"' <<<"$client")
  grim -s 1 -g "$geo" "$OUT/$slug.png"
  echo "ok   $slug ($geo)"

  kill -- -"$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -rf "$tmp"
done
