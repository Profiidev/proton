#!/usr/bin/env bash
# Captures fixtures/gpui/<slug>.png for every route of the gpui app, inside its own headless sway,
# so it works with a locked session and several invocations can run in parallel.
# Usage: fixtures/scripts/screenshots.sh [slug-filter]   e.g. `just screenshots settings`
# Env: PROTON_BIN (default target/release/proton-gpui), WAIT (seconds after the window maps, default 6: toasts autohide after 5s + 0.4s transition),
#      OUT (default fixtures/gpui)
set -euo pipefail
[[ -n ${TRACE:-} ]] && set -x

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${PROTON_BIN:-$ROOT/target/release/proton-gpui}
OUT=${OUT:-$ROOT/fixtures/gpui}
WAIT=${WAIT:-6}
FILTER=${1:-}

[[ -x $BIN ]] || (cd "$ROOT" && cargo build --release --bin proton-gpui)
mkdir -p "$OUT"

# sway/grim/swaymsg + the runtime libs gpui dlopens, all from nixpkgs (cached after the first run)
mapfile -t pkgs < <(nix build nixpkgs#sway nixpkgs#grim nixpkgs#wayland.out nixpkgs#libxkbcommon \
  nixpkgs#vulkan-loader --print-out-paths --no-link)
export PATH="${pkgs[0]}/bin:${pkgs[1]}/bin:$PATH"
LIBS=$(printf '%s/lib:' "${pkgs[@]:2}")
export LD_LIBRARY_PATH="${LIBS}${LD_LIBRARY_PATH:-}"

# own compositor: short runtime dir (unix socket paths max 108 bytes)
RUN=$(mktemp -d /tmp/claude-1000/sway.XXXXXX 2>/dev/null || mktemp -d /tmp/sway.XXXXXX)
chmod 700 "$RUN"
cat >"$RUN/config" <<'EOF'
output HEADLESS-1 resolution 1300x800 position 0 0 scale 1
default_border none
default_floating_border none
gaps inner 0
gaps outer 0
focus_on_window_activation none
# tiled (the only window fills the 1300x800 output): a floating gpui window draws its 1px
# client-side border, the Tauri reference has none
EOF
env -u WAYLAND_DISPLAY -u DISPLAY -u SWAYSOCK XDG_RUNTIME_DIR=$RUN WLR_BACKENDS=headless \
  WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=vulkan sway -c "$RUN/config" >"$RUN/sway.log" 2>&1 &
SWAY=$!
pid=""
cleanup() {
  [[ -n $pid ]] && kill -- -"$pid" 2>/dev/null
  kill "$SWAY" 2>/dev/null
  wait 2>/dev/null
  rm -rf "$RUN"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

for _ in $(seq 50); do
  sock=$(compgen -G "$RUN/wayland-*" | grep -v lock | head -1 || true)
  [[ -n $sock ]] && break
  sleep 0.1
done
[[ -n $sock ]] || { cat "$RUN/sway.log"; exit 1; }
export XDG_RUNTIME_DIR=$RUN WAYLAND_DISPLAY=${sock##*/} SWAYSOCK=$(compgen -G "$RUN/sway-ipc.*")
unset DISPLAY

# shellcheck source=routes.sh
source "$ROOT/fixtures/scripts/routes.sh"

for route in "${ROUTES[@]}"; do
  IFS='|' read -r slug path mode <<<"$route"
  [[ -n $FILTER && $slug != *$FILTER* ]] && continue
  start=$(date +%s%3N)

  tmp=$(mktemp -d)
  cp -r "$ROOT/fixtures/data/." "$tmp/"

  net=()
  [[ $mode == offline ]] && net=(unshare -n --map-current-user)
  # setsid: own process group to kill everything after; dbus-run-session: private session bus
  PROTON_ROUTE=$path XDG_DATA_HOME=$tmp XDG_CONFIG_HOME=$tmp/config XDG_CACHE_HOME=$tmp/cache \
    setsid dbus-run-session -- "${net[@]}" "$BIN" >"$tmp/stdout.log" 2>&1 &
  pid=$!

  mapped=""
  for _ in $(seq 100); do
    swaymsg -t get_tree | grep -q '"app_id": "io.profidev.proton"' && mapped=1 && break
    sleep 0.1
  done
  if [[ -z $mapped ]]; then
    echo "FAIL $slug: no window (log: $tmp/stdout.log)"
    kill -- -"$pid" 2>/dev/null || true
    pid=""
    continue
  fi

  delay=$WAIT
  [[ $mode == online ]] && delay=$((WAIT + 4))
  sleep "$delay"
  if ! swaymsg -t get_tree | grep -q '"app_id": "io.profidev.proton"'; then
    echo "FAIL $slug: window gone (crash?), log: $tmp/stdout.log"
    tail -5 "$tmp/stdout.log" | sed 's/^/     /'
    pid=""
    continue
  fi
  grim -g "0,0 1300x800" "$OUT/$slug.png"
  ms=$(($(date +%s%3N) - start))
  printf 'ok   %-45s %d.%03ds\n' "$slug" $((ms / 1000)) $((ms % 1000))

  kill -- -"$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  pid=""
  # wait for the window to unmap, so the next route can't be confused with it
  for _ in $(seq 50); do
    swaymsg -t get_tree | grep -q '"app_id": "io.profidev.proton"' || break
    sleep 0.1
  done
  rm -rf "$tmp"
done
