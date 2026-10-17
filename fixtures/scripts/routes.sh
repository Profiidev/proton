# Shared route table for screenshots.sh (sourced); reference.sh was removed with the Tauri app.
# shellcheck disable=SC2034
V=11111111-1111-4111-8111-111111111111
F=22222222-2222-4222-8222-222222222222
G=33333333-3333-4333-8333-333333333333
P=44444444-4444-4444-8444-444444444444

# slug|path|mode   mode: offline (no network namespace) or online
ROUTES=(
  "home|/home|offline"
  "profiles|/profiles|offline"
  "instances|/instances|offline"
  "instances-info-logs|/instances/info/logs?id=none|offline"
  "settings-accounts|/settings/accounts|offline"
  "settings-skins|/settings/skins|offline"
  "settings-capes|/settings/capes|offline"
  "settings-minecraft|/settings/minecraft|offline"
  "settings-java|/settings/java|offline"
  "settings-about|/settings/about|offline"
  "discover-mods|/discover/mods|online"
  "discover-modpacks|/discover/modpacks|online"
  "discover-resource-packs|/discover/resource-packs|online"
  "discover-shaders|/discover/shaders|online"
  "discover-project-sodium|/discover/project?id=AANobbMI&kind=mod&source=modrinth|online"
)
for pair in vanilla:$V fabric:$F forge:$G modpack:$P; do
  name=${pair%%:*} id=${pair#*:}
  for tab in quick_play/singleplayer quick_play/multiplayer quick_play/realms \
    content/mods content/resource-packs content/shaders instances logs \
    settings/general settings/java settings/minecraft; do
    ROUTES+=("profile-$name-${tab//\//-}|/profiles/info/$tab?id=$id|offline")
  done
done
