#!/usr/bin/env python3
"""Regenerates fixtures/data/io.profidev.proton (the committed reference data dir).

Everything is fake and deterministic. Version manifests, loader indexes and skin/cape textures are
public Mojang/loader data copied once from a real data dir (pass it as argv[1]); without it the
already committed copies are kept.
"""

import hashlib
import json
import shutil
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "data" / "io.profidev.proton"
TEX = "http://textures.minecraft.net/texture/"

SKINS = [
  ("fb1c862dba2791fcc53c491eb2ddf5f7b118850c", "4fb8a8754320876e2d2a5609e49f951310e201109e147d6f118b393dba2d6ada"),
  ("f95f506bca4c42d2ee4b8dfbda008132427f9a18", "891129157b1eb45a749b9c4169db7d5eadc7c1e85277b6a00a71cb5ced4e3382"),
]
CAPES = [
  ("5bcd3a55973e8f6243842df5f6c2ab44c709954e", "f9a76537647989f9a0b6d001e320dac591c359e9e61a31f4ce11c88f207f0ad4", "Vanilla", "7412868d-7be3-429f-b629-ed5074d2455c"),
  ("d5e012a28c6a4275bbee6cf2d7d76b7b087ef756", "28de4a81688ad18b49e735a273e086c18f1e3966956123ccb574034c06f5d336", "Pan", "2ed5269a-076e-4a3c-834a-2837d5b578f2"),
  ("a45cd618ed317cab1cb43012652382272fcb0518", "2340c0e03dd24a11b15a8b33c2a7e9e32abb2051b2481d0ba7defd635ca7a933", "Migrator", "5af20372-79e0-4e1f-80f8-6bd8e3135995"),
  ("061f700c94ca82f109fe045e332425201218e206", "5ec930cdd2629c8771655c60eebeb867b4b6559b0e6d3bc71c40c96347fa03f0", "Common", "b9d4f2e0-6109-43f7-97aa-84250ce3c1dd"),
]
COPY = [
  "minecraft/manifest.json",
  "java/manifest.json",
  *[f"minecraft/versions/{n}" for n in ("fabric-game.json", "fabric-loader.json", "forge-index.json", "neoforge-index.json", "quilt-game.json", "quilt-loader.json")],
  *[f"skins/{i}.png" for i, _ in SKINS],
  *[f"skins/{i}_head.png" for i, _ in SKINS],
  *[f"skins/{c[0]}.png" for c in CAPES],
]

VANILLA = "11111111-1111-4111-8111-111111111111"
FABRIC = "22222222-2222-4222-8222-222222222222"
FORGE = "33333333-3333-4333-8333-333333333333"
PACK = "44444444-4444-4444-8444-444444444444"
ACC1 = "f1f1f1f1f1f14f1f8f1f1f1f1f1f1f1f"
ACC2 = "a2a2a2a2a2a24a2a8a2a2a2a2a2a2a2a"


def png(path: Path, w: int, h: int, c1, c2):
  """Diagonal two-color gradient PNG, no dependencies."""
  rows = b""
  for y in range(h):
    rows += b"\0"
    for x in range(w):
      t = (x + y) / (w + h - 2)
      rows += bytes(round(a + (b - a) * t) for a, b in zip(c1, c2))

  def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

  path.parent.mkdir(parents=True, exist_ok=True)
  path.write_bytes(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows, 9))
    + chunk(b"IEND", b"")
  )


def write(path: Path, data):
  path.parent.mkdir(parents=True, exist_ok=True)
  if isinstance(data, (dict, list)):
    data = json.dumps(data, indent=2) + "\n"
  path.write_text(data)


def account(pid, name, skin, capes, active_cape):
  far = "2099-01-01T00:00:00Z"
  return {
    "auth": {
      "mc_token": f"fixture-mc-token-{name}",
      "mc_token_expire": far,
      "user_hash": "fixture",
      "xbox_security_token": "fixture",
      "xbox_security_token_expire": far,
      "xbox_token": "fixture",
      "xbox_token_expire": far,
      "ms_access_token": "fixture",
      "ms_access_token_expire": far,
      "ms_refresh_token": "fixture",
    },
    "profile": {
      "id": pid,
      "name": name,
      "skins": [{"id": f"{pid[:8]}-0000-4000-8000-000000000000", "state": "ACTIVE", "url": TEX + skin[1], "textureKey": skin[1], "variant": "CLASSIC"}],
      "capes": [
        {"id": c[3], "state": "ACTIVE" if c[2] == active_cape else "INACTIVE", "url": TEX + c[1], "alias": c[2]} for c in capes
      ],
    },
  }


def quick_play(id, name, kind, when, favorite=False, history=False):
  return {"id": id, "name": name, "lastPlayedTime": when, "favorite": favorite, "history": history, "type": kind}


def profile(id, name, version, loader, loader_version, created, last_played=None, favorite=False, quick=(), downloaded=True, modpack=None, local_jvm=None):
  return {
    "id": id,
    "name": name,
    "created_at": created,
    "last_played": last_played,
    "last_played_non_quick_play": last_played,
    "favorite": favorite,
    "quick_play": list(quick),
    "version": version,
    "loader": loader,
    "loader_version": loader_version,
    "downloaded": downloaded,
    "use_local_game": False,
    "game": None,
    "use_local_jvm": local_jvm is not None,
    "jvm": local_jvm,
    "modpack": modpack,
  }


def entry(kind, file, name=None, project=None, version=None, number=None, icon=None, source="modrinth", from_pack=False, manual=False, channel="release"):
  known = project is not None
  return {
    "kind": kind,
    "fileName": file,
    "sha1": hashlib.sha1(file.encode()).hexdigest(),
    "fingerprint": sum(file.encode()) * 7919 % 2**32,
    "manual": manual,
    "fromPack": from_pack,
    "installedAt": "2026-10-01T12:00:00.000000000+00:00" if known else None,
    "source": source if known else None,
    "projectId": project,
    "versionId": version,
    "name": name,
    "iconUrl": icon,
    "versionNumber": number,
    "channel": channel if known else None,
    "dependencyOf": [],
    "pending": False,
  }


def content(dir: Path, entries, disabled=()):
  for e in entries:
    folder = {"mod": "mods", "resourcepack": "resourcepacks", "shader": "shaderpacks"}[e["kind"]]
    name = e["fileName"] + (".disabled" if e["fileName"] in disabled else "")
    write(dir / folder / name, f"fixture placeholder for {e['fileName']}\n")
  write(dir / "proton-content.json", {"version": 1, "entries": entries})


def log(dir: Path, stamp: str, mc: str, loader: str, crash=False):
  hh = stamp[11:13]
  lines = [
    f"[{hh}:00:01] [main/INFO]: Loading Minecraft {mc} with {loader}",
    f"[{hh}:00:01] [main/INFO]: Setting user: Steve_Fixture",
    f"[{hh}:00:02] [Render thread/INFO]: Backend library: LWJGL version 3.3.3",
    f"[{hh}:00:02] [Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play",
    f"[{hh}:00:03] [Render thread/INFO]: Reloading ResourceManager: vanilla, fabric",
    f"[{hh}:00:04] [Worker-Main-2/INFO]: Preparing spawn area: 0%",
    f"[{hh}:00:05] [Worker-Main-2/INFO]: Preparing spawn area: 83%",
    f"[{hh}:00:06] [Server thread/INFO]: Steve_Fixture joined the game",
    f"[{hh}:00:07] [Render thread/INFO]: [System] [CHAT] Hello from the fixture log",
  ]
  if crash:
    lines += [
      f"[{hh}:00:08] [Render thread/ERROR]: Reported exception thrown!",
      "java.lang.NullPointerException: Cannot invoke \"Object.toString()\" because \"value\" is null",
      "\tat net.minecraft.client.Minecraft.run(Minecraft.java:812)",
      "\tat net.minecraft.client.main.Main.main(Main.java:251)",
    ]
  else:
    lines += [f"[{hh}:00:30] [Render thread/INFO]: Stopping!"]
  write(dir / "instance_logs" / f"{stamp}.log", "\n".join(lines) + "\n")


def main():
  src = Path(sys.argv[1]) if len(sys.argv) > 1 else None
  keep = {}
  for rel in COPY:
    f = ROOT / rel
    if src:
      keep[rel] = (src / rel).read_bytes()
    elif f.exists():
      keep[rel] = f.read_bytes()
  if ROOT.exists():
    shutil.rmtree(ROOT)
  for rel, data in keep.items():
    (ROOT / rel).parent.mkdir(parents=True, exist_ok=True)
    (ROOT / rel).write_bytes(data)

  profiles_dir = ROOT / "profiles"
  t = "2026-10-1{}T{}:00:00.123456789Z"

  # vanilla: favorite, quick play of every type, resource packs, two runs
  v = profiles_dir / VANILLA
  write(v / "profile.json", profile(
    VANILLA, "Vanilla Survival", "1.21.8", "Vanilla", None, t.format(0, 10), t.format(5, 18), favorite=True,
    quick=[
      quick_play("New World", "New World", "singleplayer", t.format(5, 18), favorite=True, history=True),
      quick_play("Creative Test", "Creative Test", "singleplayer", t.format(4, 20), history=True),
      quick_play("mc.hypixel.net", "Hypixel", "multiplayer", t.format(3, 21), favorite=True, history=True),
      quick_play("play.example.org:25565", "Example SMP", "multiplayer", t.format(2, 19)),
      quick_play("1234567", "Fixture Realm", "realms", t.format(1, 17), history=True),
    ],
  ))
  png(v / "image.png", 64, 64, (70, 160, 70), (120, 80, 40))
  for world, colors in (("New World", ((90, 170, 255), (40, 120, 40))), ("Creative Test", ((240, 200, 90), (200, 90, 40)))):
    write(v / "saves" / world / "level.dat", "fixture\n")
    png(v / "saves" / world / "icon.png", 64, 64, *colors)
  content(v, [
    entry("resourcepack", "Faithful 32x.zip", "Faithful 32x", "FTBCIDcX", "fxtFaith", "1.21.8-r1"),
    entry("resourcepack", "Fresh Animations.zip", "Fresh Animations", "50dA9Sha", "fxtFresh", "1.9.2"),
    entry("resourcepack", "My Own Pack.zip", manual=True),
  ])
  write(v / "options.txt", 'version:4440\nresourcePacks:["vanilla","file/Fresh Animations.zip","file/Faithful 32x.zip"]\nlang:en_us\n')
  log(v, "2026-10-14T20-00-00.123456789+00-00", "1.21.8", "Vanilla")
  log(v, "2026-10-15T18-00-00.123456789+00-00", "1.21.8", "Vanilla", crash=True)

  # fabric: mods (one disabled, one manual), shaders with an active pack
  f = profiles_dir / FABRIC
  write(f / "profile.json", profile(FABRIC, "Fabric Performance", "1.21.8", "Fabric", "0.17.2", t.format(1, 9), t.format(4, 12), local_jvm={"args": ["-XX:+UseG1GC"], "env_vars": {}, "mem_max": 6144}))
  png(f / "image.png", 64, 64, (200, 180, 140), (60, 60, 90))
  content(f, [
    entry("mod", "fabric-api-0.129.0+1.21.8.jar", "Fabric API", "P7dR8mSH", "fxtFapi1", "0.129.0+1.21.8"),
    entry("mod", "sodium-fabric-0.6.13+mc1.21.8.jar", "Sodium", "AANobbMI", "fxtSodi1", "mc1.21.8-0.6.13-fabric"),
    entry("mod", "lithium-fabric-0.18.0+mc1.21.8.jar", "Lithium", "gvQqBUqZ", "fxtLith1", "mc1.21.8-0.18.0-fabric"),
    entry("mod", "iris-fabric-1.9.1+mc1.21.8.jar", "Iris Shaders", "YL57xq9U", "fxtIris1", "1.9.1+1.21.8-fabric"),
    entry("mod", "modmenu-15.0.0.jar", "Mod Menu", "mOgUt4GM", "fxtMenu1", "15.0.0", channel="beta"),
    entry("mod", "debugify-1.21.8+1.0.jar", "Debugify", "QwxR6Gcd", "fxtDebu1", "1.21.8+1.0"),
    entry("mod", "local-tweaks-1.0.jar", manual=True),
    entry("shader", "ComplementaryReimagined_r5.5.1.zip", "Complementary Reimagined", "HVnmMxH1", "fxtComp1", "r5.5.1"),
    entry("shader", "BSL_v10.0.zip", "BSL Shaders", "Q1vvjJYV", "fxtBsl01", "v10.0"),
  ], disabled={"debugify-1.21.8+1.0.jar"})
  write(f / "config" / "iris.properties", "enableShaders=true\nshaderPack=ComplementaryReimagined_r5.5.1.zip\n")
  log(f, "2026-10-14T12-00-00.123456789+00-00", "1.21.8", "Fabric Loader 0.17.2")

  # forge: not downloaded yet, curseforge mods
  g = profiles_dir / FORGE
  write(g / "profile.json", profile(FORGE, "Forge Classic", "1.20.1", "Forge", "47.3.0", t.format(2, 15), downloaded=False))
  png(g / "image.png", 64, 64, (60, 70, 110), (200, 120, 60))
  content(g, [
    entry("mod", "jei-1.20.1-forge-15.20.0.106.jar", "Just Enough Items (JEI)", "238222", "5846880", "15.20.0.106", source="curseforge"),
    entry("mod", "journeymap-1.20.1-5.10.3-forge.jar", "JourneyMap", "32274", "5789363", "5.10.3", source="curseforge"),
    entry("mod", "create-1.20.1-0.5.1.j.jar", "Create", "328085", "5838779", "0.5.1.j", source="curseforge"),
  ])

  # modpack based: Fabulously Optimized, content from the pack
  p = profiles_dir / PACK
  write(p / "profile.json", profile(
    PACK, "Fabulously Optimized", "1.21.8", "Fabric", "0.17.2", t.format(3, 8), t.format(5, 9), favorite=True,
    modpack={"source": "modrinth", "projectId": "1KVo5zza", "versionId": "fxtFoPck"},
  ))
  png(p / "image.png", 64, 64, (230, 90, 150), (90, 40, 160))
  content(p, [
    entry("mod", "sodium-fabric-0.6.13+mc1.21.8.jar", "Sodium", "AANobbMI", "fxtSodi1", "mc1.21.8-0.6.13-fabric", from_pack=True),
    entry("mod", "entityculling-fabric-1.8.2-mc1.21.8.jar", "Entity Culling", "NNAgCjsB", "fxtEnti1", "1.8.2", from_pack=True),
    entry("mod", "ImmediatelyFast-Fabric-1.11.1+1.21.8.jar", "ImmediatelyFast", "5ZwdcRci", "fxtImme1", "1.11.1+1.21.8-fabric", from_pack=True),
    entry("mod", "continuity-3.0.0+1.21.8.jar", "Continuity", "1IjD5062", "fxtCont1", "3.0.0+1.21.8", from_pack=True),
    entry("resourcepack", "FO Default Dark Mode.zip", "Default Dark Mode", "6SLU7tS5", "fxtDark1", "1.21.8", from_pack=True),
  ])
  write(p / "options.txt", 'resourcePacks:["vanilla","file/FO Default Dark Mode.zip"]\n')
  log(p, "2026-10-15T09-00-00.123456789+00-00", "1.21.8", "Fabric Loader 0.17.2")

  write(ROOT / "store.json", {
    "accounts": {
      ACC1: account(ACC1, "Steve_Fixture", SKINS[0], CAPES, "Vanilla"),
      ACC2: account(ACC2, "Alex_Fixture", SKINS[1], CAPES[1:2], None),
    },
    "active_account": ACC1,
    "skins": [{"id": i, "url": TEX + k} for i, k in SKINS],
    "capes": [{"id": c[0], "url": TEX + c[1]} for c in CAPES],
    "profiles": {id: f"profiles/{id}" for id in (VANILLA, FABRIC, FORGE, PACK)},
    "content_updates_checked": "2099-01-01T00:00:00Z",
    "settings": {
      "system_max_mem": 16384,
      "sidebar_width": None,
      "url": "tauri://localhost/home",
      "auto_update": False,
      "hide_window_controls": False,
      "download_dirs": [],
      "minecraft": {
        "show_snapshots": False,
        "game_settings": {"use_custom": False, "width": 854, "height": 480},
        "jvm_settings": {"args": [], "env_vars": {}, "mem_max": 4096},
      },
    },
  })


if __name__ == "__main__":
  main()
