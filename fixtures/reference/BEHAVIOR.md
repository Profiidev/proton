# Proton UI functional spec (reference for the gpui rewrite)

The PNGs in this directory are frozen: they were captured from the Tauri app, which was deleted at the gpui cutover. Its last source is commit `d7dab77`, and the capture script is `fixtures/scripts/reference.sh` at commit `10acfd2`.

Source of truth: `app/src` at commit `d7dab77` (deleted since; SvelteKit + Svelte 5 + @profidev/pleiades/shadcn-svelte). Strings in quotes are exact UI text. `cmd(...)` = Tauri invoke command name. "data-state" = value auto-refetched when backend emits `data-update` event with the matching `UpdateType` payload (see Shared > Data refresh).

---

## 1. Theme tokens

- Default theme: **dark**. `+layout.svelte` calls `setMode('dark')` (mode-watcher) on startup; no theme toggle exists anywhere in the UI. Light tokens only matter for completeness.
- Font: `@fontsource-variable/geist` → `--font-sans` / `--font-heading` = `'Geist Variable', sans-serif`. Monospace = Tailwind default `font-mono` (logs, account ids, env vars).
- Imports: `tailwindcss`, `tw-animate-css`, `shadcn-svelte/tailwind.css`. Dark variant = `.dark *`.
- Base: every element `border-border`, `outline-ring/50`; `body` bg-background / text-foreground. Pointer cursor on all interactive elements (links, buttons, tabs, switches, checkboxes, comboboxes, sliders, toggle items, labels) unless disabled.
- Buttons have a pressed translate (shadcn); nested-button press does not move the outer button.

### Radius

- `--radius: 0.625rem` (10px)
- sm = r*0.6 (6px), md = r*0.8 (8px), lg = r (10px), xl = r*1.4 (14px), 2xl = r*1.8, 3xl = r*2.2, 4xl = r*2.6

### Colors (light `:root` / dark `.dark`)

| token                      | light                      | dark                       |
| -------------------------- | -------------------------- | -------------------------- |
| background                 | oklch(1 0 0)               | oklch(0.141 0.005 285.823) |
| foreground                 | oklch(0.141 0.005 285.823) | oklch(0.985 0 0)           |
| card                       | oklch(1 0 0)               | oklch(0.21 0.006 285.885)  |
| card-foreground            | oklch(0.141 0.005 285.823) | oklch(0.985 0 0)           |
| popover                    | oklch(1 0 0)               | oklch(0.21 0.006 285.885)  |
| popover-foreground         | oklch(0.141 0.005 285.823) | oklch(0.985 0 0)           |
| primary                    | oklch(0.488 0.243 264.376) | oklch(0.424 0.199 265.638) |
| primary-foreground         | oklch(0.97 0.014 254.604)  | oklch(0.97 0.014 254.604)  |
| secondary                  | oklch(0.967 0.001 286.375) | oklch(0.274 0.006 286.033) |
| secondary-foreground       | oklch(0.21 0.006 285.885)  | oklch(0.985 0 0)           |
| muted                      | oklch(0.967 0.001 286.375) | oklch(0.274 0.006 286.033) |
| muted-foreground           | oklch(0.552 0.016 285.938) | oklch(0.705 0.015 286.067) |
| accent                     | oklch(0.967 0.001 286.375) | oklch(0.274 0.006 286.033) |
| accent-foreground          | oklch(0.21 0.006 285.885)  | oklch(0.985 0 0)           |
| destructive                | oklch(0.577 0.245 27.325)  | oklch(0.704 0.191 22.216)  |
| border                     | oklch(0.92 0.004 286.32)   | oklch(1 0 0 / 10%)         |
| input                      | oklch(0.92 0.004 286.32)   | oklch(1 0 0 / 15%)         |
| ring                       | oklch(0.705 0.015 286.067) | oklch(0.552 0.016 285.938) |
| chart-1                    | oklch(0.809 0.105 251.813) | same                       |
| chart-2                    | oklch(0.623 0.214 259.815) | same                       |
| chart-3                    | oklch(0.546 0.245 262.881) | same                       |
| chart-4                    | oklch(0.488 0.243 264.376) | same                       |
| chart-5                    | oklch(0.424 0.199 265.638) | same                       |
| sidebar                    | oklch(0.985 0 0)           | oklch(0.21 0.006 285.885)  |
| sidebar-foreground         | oklch(0.141 0.005 285.823) | oklch(0.985 0 0)           |
| sidebar-primary            | oklch(0.546 0.245 262.881) | oklch(0.623 0.214 259.815) |
| sidebar-primary-foreground | oklch(0.97 0.014 254.604)  | oklch(0.97 0.014 254.604)  |
| sidebar-accent             | oklch(0.967 0.001 286.375) | oklch(0.274 0.006 286.033) |
| sidebar-accent-foreground  | oklch(0.21 0.006 285.885)  | oklch(0.985 0 0)           |
| sidebar-border             | oklch(0.92 0.004 286.32)   | oklch(1 0 0 / 10%)         |
| sidebar-ring               | oklch(0.705 0.015 286.067) | oklch(0.552 0.016 285.938) |

### Recurring non-token colors (Tailwind palette)

- Running/ok chip: `bg-green-500/15 text-green-500`, dot `bg-green-500` (size 6px or 8px, round).
- Update/badge chip: `bg-amber-500/15 text-amber-500`; version-update text `text-yellow-500`.
- Favorite star: `fill-yellow-500 text-yellow-500`.
- Primary chip: `bg-primary/15 text-primary`.
- Neutral chip: `bg-secondary`, h-5 (20px), px-2, rounded-full, text-xs font-medium.
- Card style ("ring card"): `bg-card ring-1 ring-foreground/10 rounded-xl`.
- Tile style ("input tile"): `border border-input bg-input/30 hover:bg-input/60 rounded-lg`.
- List container: `bg-card/50 border rounded-xl overflow-hidden`, rows `border-b last:border-0 hover:bg-accent/40`.
- Log window bg: `color-mix(in oklab, var(--background) 60%, black)`.

### Custom CSS

- `.prose-content` (project description HTML): text-sm leading-relaxed break-words; sibling spacing mt-3; h1–h4 mt-5 semibold (h1 text-lg, h2 text-base); links text-primary underline offset-2; ul disc / ol decimal pl-5; img inline max-w-full rounded-md; code/pre bg-muted rounded-sm mono text-xs (code px-1 py-0.5, pre p-3 overflow-x-auto); blockquote muted border-l-2 pl-3; tables block overflow-x-auto, th/td border px-2 py-1.
- No custom keyframes. Animations used: `animate-spin` (LoaderCircle / Spinner), `transition-colors`, svelte `crossfade` (see sidebar), shadcn dialog/popover/dropdown default enter/exit (tw-animate-css fade+zoom).

---

## 2. App shell

### Window

- Tauri window: title "Proton", 1300×800, min 600×400, `decorations: false` (custom titlebar), starts hidden; frontend calls `getCurrentWebviewWindow().show()` on mount.
- Layout: column [Header (h-40px)] / Separator / row [Sidebar | vertical Separator | main (grow, overflow-auto)].
- Routes: `/` → 308 redirect `/home`; `/discover` → `/discover/mods`; `/settings` → `/settings/accounts`.

### Header (`nav/Header.svelte`)

- Whole header is a drag region (`data-tauri-drag-region`), h-10, text-sm.
- Left: app icon `/icon.svg` (28px, mx-2), "Proton" (font-medium, mr-4), vertical separator (h-5), breadcrumbs (mx-4, no wrap, overflow hidden).
- Right (gap-1, pr-1):
  - If offline: destructive Badge "Offline" (rounded-full, pointer). Click → `cmd(try_reconnect)` → toast success "Reconnected successfully" or error "Failed to reconnect".
  - DownloadsPanel chip.
  - Unless setting `hide_window_controls`: three ghost round 32px icon buttons: Minimize (lucide `Minus`, 18px) → minimize; Maximize (`Square`) → toggleMaximize; Close (`X`, 18px, hover bg-destructive) → close window.

### Breadcrumbs (`nav/breadcrumbs.ts`)

- Segment label = split on `_`/`-`, capitalize each word, join space (`quick_play` → "Quick Play", `resource-packs` → "Resource Packs").
- Path `/discover/project?kind&id`: ["Discover" (/discover), Title(KIND_ROUTE[kind]) e.g. "Mods" (/discover/mods) if kind valid, project name from cache or "Project"].
- Otherwise one crumb per segment. Segment `info` at index 1: under profiles → profile name (or "Profile"); under instances → instance's profile_name (or "Instance"). Crumbs at depth ≥1 below an `info` route keep the query string (`?id=`).
- Last crumb = plain page text; others = links. Separators between.

### Sidebar (`nav/Sidebar.svelte`)

- `nav` w-16 (64px) below md, md:w-54 (216px) at ≥768px; p-2, gap-0.5. Width is fixed by breakpoint (setting `sidebar_width` exists in Settings type but is unused by the frontend).
- Items (icon lucide, 20px):
  - "Home" `/home` `Home`
  - "Profiles" `/profiles` `LibraryBig`
  - "Instances" `/instances` `Gamepad2` — when ≥1 running instance (md+ only): green pill right-aligned with dot + count.
  - Section label "Discover" (md+: text-xs muted, mt-3 mb-1 px-2.5; below md: a Separator instead)
  - "Mods" `/discover/mods` `Puzzle`; "Modpacks" `/discover/modpacks` `Package`; "Resource packs" `/discover/resource-packs` `Image`; "Shaders" `/discover/shaders` `Sparkles`
  - flex spacer (min 16px)
  - "Settings" `/settings` `Settings`
  - Separator, AccountSelector
- Item: h-10, rounded-md, px-2.5, gap-3, text-sm medium, muted-foreground; hover `bg-accent/60 text-foreground`; labels hidden below md (icon only, 40×40 centered).
- Active = pathname startsWith item url, or on `/discover/project` the item for `KIND_ROUTE[kind]`. Active text-foreground + `bg-muted` rounded-md indicator behind.
- Active indicator animation: svelte `crossfade({ duration: 250, easing: cubicInOut })`, shared key `active-sidebar-tab` → indicator slides/morphs from old to new item in 250ms cubic-in-out. Same in SimpleSidebar (separate crossfade instance, same params/key).

### DownloadsPanel (`nav/DownloadsPanel.svelte`)

- Hidden when no jobs. Chip trigger: `bg-muted` h-7 rounded-full px-2.5 text-xs, `Download` icon 14px, label `chip_label`: 0 running → "Downloads", 1 → "1 download", n → "n downloads"; when running>0 also a 40px Progress bar = average percent of running jobs.
- Popover (align end, width 400px): header "Downloads" + ghost xs button "Clear finished" (disabled when all are running; removes non-running). List max-h 384px scroll; per job: title (truncate, medium), percent label, cancel icon button `X` "Cancel download" (only running; disabled after requested → `cmd(profile_cancel_download,{id})`), Progress bar, row: detail text (left) + speed (right). Footer text: "Downloads continue while you browse. Games start as soon as their files are ready."
- Percent label: done "Done", error "Failed", canceled "Canceled", running `${pct}%` or "" if no value.
- Detail (running only): stage text, or `${stage} · ${mib(v)} of ${mib(t)} MiB` (byte stages; mib 1 decimal if <10 else integer) or `${stage} · ${v} of ${t}[ unit]` (en-US grouping). Speed: `${(bytes/s / 1e6).toFixed(1)} MB/s` (smoothed EMA weight 0.3), only for byte stages.
- Progress events: `version-check-status` {id, data}. Stage texts: VersionManifestCheck "Checking version manifest", VersionManifestDownload "Downloading version manifest", AssetsManifestCheck "Checking assets manifest", AssetsManifestDownload "Downloading assets manifest", JavaManifestCheck "Checking java manifest", JavaManifestDownload "Downloading java manifest", ClientCheck "Checking client jar", ModLoaderMeta "Downloading mod loader version meta", ModLoaderFilesDownloadInfo "Downloading mod loader file information", ModLoaderPreprocess "Running mod loader preprocessing", ModLoaderPreprocessDone "Mod loader preprocessing done", ContentResolve "Resolving content", Done "Starting" (sets 100%). Progress stages `[value,total]`: ClientDownload "Downloading client", AssetsCheck "Checking assets", AssetsDownload "Downloading assets", JavaCheck "Checking java files", JavaDownload "Downloading java files", NativeLibraryCheck "Checking native libraries", NativeLibraryDownload "Downloading native libraries", LibraryCheck "Checking libraries", LibraryDownload "Downloading libraries", ModLoaderFilesCheck "Checking mod loader files", ModLoaderFilesDownload "Downloading mod loader files", ContentDownload "Downloading content", ContentFiles "Installing content" (unit "files"), ModpackOverrides "Applying modpack overrides". Keys ending in `Download` are byte stages. New job initial stage "Checking version manifest".

### Jobs (`run_job` in tauri/profile.svelte.ts)

- Generic: adds job to downloads list with title, invokes `cmd(name, {...args, id})` (id = monotonically increasing from Date.now()), then: throw → mark error + toast.error(err); return but cancel was requested → toast.warning "Check/Download canceled"; else toast.success(ok).
- Used by: launch (`profile_launch`), repair, content install/update, modpack install/update, export (titles/messages listed where used).

### Toaster

- svelte-sonner, `position="top-right"`, `closeButton`, `richColors`, theme follows mode (dark). Icons: loading spinner, success CircleCheck, error OctagonX, info Info, warning TriangleAlert (16px). Default duration (sonner 4s) unless stated.
- Global toasts:
  - Startup offline (`cmd(is_offline)` true at mount): warning "You are currently offline, some features may not work" (10000ms).
  - Startup `cmd(account_refresh)` fails: error "Failed to refresh Accounts".
  - `instance-crash` event {profile_name}: warning "An instance of profile {profile_name} has crashed."
  - `manifest-refresh-error` event: error "Failed to refresh manifests."
  - Update check fails: error "Failed to check for updates".
  - `account-login-status` only handled on Accounts page (see there). `version-check-status` only feeds DownloadsPanel (no toast).
- `TOAST_DURATION` = 600000ms (10 min) for long-running loading toasts.

### Updater flow

- On mount: `checkForUpdates()` (tauri updater `check()`). If version found: if `settings.auto_update` → `update()` immediately; else toast.message "Update available: {version}" with action button "Update" → navigates `/settings/about`.
- `update()`: downloadAndInstall; Started → loading toast "Downloading update: 0%" (10 min); Progress → same toast "Downloading update: {pct}%"; Finished → dismiss, success "Update downloaded successfully. Restarting ..."; then `relaunch()`. Failure → dismiss + error "Failed to download update".

### Persisted last URL

- On every route change, debounced 500ms: `settings.url = page.url; cmd(settings_set)`. (Backend uses it to restore on start.)

### Keyboard shortcuts

- None custom. Only native: Enter submits forms (create profile, duplicate, env var add), Escape closes dialogs/popovers, arrow keys in radio groups/selects/command lists, Tab focus.

### Offline

- `cmd(is_offline)` data-state (UpdateType Offline). Shows header "Offline" badge. Some actions guarded with warning toasts (repair, add account, change skin/cape, check updates).

---

## 3. Routes

Common page padding: `px-5 pt-4 pb-6`, vertical gap-4/6. Page H1: text-xl semibold tracking-tight. Section H2: text-base medium. Muted subtitle text-sm or default.

### /home

- Data: `cmd(profile_favorites_list)`, `cmd(profile_history_list)` (both `{profile, quick_play?}[]`, UpdateType Profiles), `cmd(profile_list)`, `cmd(profile_mods_count)`, `cmd(instance_list)`, `cmd(account_get_active)`, `cmd(content_updates)`, `cmd(modpack_updates)`.
- **Continue playing card** (only if any profile has `last_played`): ring card p-4 flex-wrap. `current` = most recently played profile; `current_quick_play` = its newest quick_play entry if played after `last_played_non_quick_play` (or none plain).
  - Icon (56px, rounded-lg, fallback Box 28px) links to `/profiles/info/quick_play/{qp.type ?? singleplayer}?id=`.
  - Overline "Continue playing" (xs, uppercase, tracking-wider, muted); title = qp name or profile name (text-xl semibold); meta joined " · ": [profile name if qp, "{loader} {version}", "{n} mod(s)" if modded and count known, running_label if running].
  - Button (primary, Play icon): "Play" or "Launch another" if running → `profile_launch(id, name, active, qp)`.
- **Content updates banner** (Alert, CircleArrowUp) when `content_updates` has entries for any profile: title "{n} mod update(s) available" (word "mod update" if all are mods, else "update"); description "{A}, {B} and {C} has/have newer compatible versions."; action outline button "Review updates" → `/profiles/info/content/{kind route of first update}?id={first affected}`.
- **Modpack updates banner** when any profile in `modpack_updates`: title "{n} modpack update(s) available", description "{names joined ', '} has/have a new pack version.", button "Review" → `/profiles/info?id={first}`.
- **Favorites** section: header "Favorites" + count pill (bg-secondary). Grid `repeat(auto-fill, minmax(264px,1fr))` gap-2 of ProfileCard (profile, quick_play). Sort: compareProfiles (played desc, then never-played by created desc). Empty: "No favorites found. Add a profile or quick play option to your favorites in the profile settings." + ProfileExternalLink button.
- **History** section: header "History" + muted xs "Recently played". Same grid/sort. Empty: "No entries found in history. Launch a profile to create one." + ProfileExternalLink.

### /profiles

- Data: `cmd(profile_list)`, `cmd(version_list,{loader:'Vanilla'})`.
- Header: H1 "Profiles" + muted summary "{n} profile(s) · {m} favorite(s)".
- Toolbar (wrap, gap-2):
  - Search input with Search icon, placeholder "Search profiles by name, loader or version" (type search, min-w 200px, flex 1 1 280px). Fuzzy search with fuse.js: keys name (weight 1), version (0.5), loader (0.5); `threshold: 0.4`, `useExtendedSearch: true`. No debounce (live).
  - Multiselect "Loader" (options = distinct loaders of profiles; button shows " Loader" when empty / selected labels joined).
  - Multiselect "Version" (options = vanilla versions that some profile uses, in manifest order).
  - Primary button `Plus` "New profile" → opens Create dialog.
- Filters AND-ed, result sorted by compareProfiles. Grid of ProfileCard (264px min). Empty: "No profiles found. Adjust your filters or create a new profile."
- **Create profile dialog**: width min(620px, 100vw-32px). Title "Create profile", description "Pick a loader and version, or start from a modpack." Tabs: "Custom" | "From modpack" | "Import file" (default Custom; tab contents for modpack/import only mounted while active).
  - Selecting a modpack or importing a plan closes this dialog and opens ModpackInstallDialog.

#### CreateProfileForm (Custom tab)

- Row: FormImage icon picker (72px, dashed border, placeholder text "Icon", Upload icon; accept png/jpeg/jpg) + "Name" input placeholder "Survival SMP".
- "Loader" radio grid of 5 equal columns: Vanilla, Fabric, Quilt, Forge, NeoForge (h-9 tiles, 13px medium; checked = `border-primary bg-primary/35`). Default Vanilla.
- Row: "Minecraft version" VersionSelect (options `cmd(version_list,{loader})`, refetched on loader or show_snapshots change; auto-select first if current not in list) | "Loader version" VersionSelect (`cmd(loader_version_list,{loader, mcVersion})`; disabled & cleared for Vanilla/empty; auto-select first).
- Small switch "Show snapshots" (bound to global `settings.minecraft.show_snapshots`; saving failure → toast error "Failed to save Minecraft settings").
- Hint (xs muted): "Minecraft, Java and the loader download when you first press play."
- Footer: outline "Cancel" (close), primary "Create profile" (disabled while submitting, spinner shown).
- Validation (zod): name min 1 "Name is required"; version min 1 "Version is required"; icon must be File "Icon must be a file", type png/jpeg/jpg else "Invalid image file type"; loader enum. Errors shown under fields.
- Submit → `cmd(profile_create,{name, version, loader, loader_version?, icon?: bytes})`. Error InvalidImage → field error "Invalid image"; other error → toast "Failed to create profile"; success → close dialog.

#### FromModpackTab

- Search input placeholder "Search modpacks" (debounce 300ms, trimmed). Query `content_search` kind modpack, limit 10, sort "relevance" if query else "downloads", sources from `cmd(content_sources)`.
- List area h-80 (320px) scroll: failed → "Search failed. Check your connection and try again."; loading → centered spinner; empty → "No modpacks found."; rows: ProjectIcon 36px, name, sub "loader · loader · {newest game version}" (capitalize), "{compact downloads} downloads", outline sm button "Select" → onselect(project).

#### ImportFileTab

- Dashed drop box h-48 (192px), rounded-xl; while dragging over window: `border-primary bg-primary/10`. Content: FileUp icon, "Drop an .mrpack or .zip modpack file here", outline "Choose file" (native open dialog filter "Modpack" mrpack/zip). Busy: spinner + "Reading modpack…".
- Load → `cmd(modpack_plan,{source:{type:'path',path}})` → success: onplan(plan); InvalidPack: "This file is not a supported modpack"; other: "Failed to read the modpack file". Dropping another extension: "Only .mrpack and .zip files can be imported". Errors shown below in destructive text-sm.

### /profiles/info (layout) `?id=`

- `id` from query. `/profiles/info` → 302 `/profiles/info/quick_play?id=` → `/profiles/info/quick_play/singleplayer?id=`. If profile list loaded and id not found → navigate `/profiles`. While loading: "Loading...".
- Header (border-b, pb-4, wrap): ProfileIcon 80px rounded-lg (fallback Box 40px); name (text-2xl semibold) + running chip (green, dot, "Running"/"{n} running") + pack update button chip (primary/15, CircleArrowUp, "Pack update {latest.versionNumber}" → opens change-version dialog); line "{loader} {version}" (muted sm); line xs muted "Created {date} · Last played {date|'Never'}" (dates luxon locale `de`, DATETIME_SHORT e.g. "16.10.2026, 14:30").
- Actions:
  - Not running: primary `Play` "Play" → `profile_launch(id,name,active)`.
  - Running: outline `Plus` "Launch another"; destructive `Square` "Stop" / "Stop all" (n>1) → confirm dialog.
  - Icon outline button "Toggle favorite" (Star; filled yellow when favorite) → `cmd(profile_favorites_set,{profile, favorite:!fav})`.
  - Ellipsis menu "Profile menu" (align end, width 210px):
    - `FolderOpen` "Open directory" → `cmd(profile_open_path)`
    - `Wrench` "Repair profile" → offline: warning "You are currently offline, please reconnect to the internet to repair a profile"; else job `profile_repair` title "{name}: repair", ok "Repair of profile {name} complete", err "Failed to repair profile {name}".
    - `RefreshCw` "Change pack version" (only if `profile.modpack`) → ModpackInstallDialog in change mode.
    - `Copy` "Duplicate profile" → DuplicateDialog.
    - `Download` "Export as .mrpack" → native save dialog default "{name}.mrpack", filter "Modrinth pack" (mrpack) → job `profile_export_mrpack` {include:['config','options.txt'], path} title "{name}: export", ok "Exported {name}", err "Failed to export {name}".
    - destructive `Trash` "Delete profile" → DestroyDialog.
- Body: left aside w-40 (160px) SimpleSidebar; right content.
  - "Quick play" `FastForward` → quick_play; "Content" `Package` → content/mods (active for any /content path; amber badge = number of content updates for profile); "Instances" `Monitor`; "Logs" `Terminal`; "Settings" `SlidersHorizontal`.
- Dialogs:
  - Delete: title "Delete Profile", desc `Are you sure you want to delete the profile "{name}"? This action cannot be undone.`, button "Delete" → `cmd(profile_remove)` (layout then redirects to /profiles as profile disappears).
  - Stop: title "Stop Instance" / "Stop All Instances"; desc `Are you sure you want to stop the instance of profile "{name}"?` / `Are you sure you want to stop all {n} instances of profile "{name}"?`; button "Stop"/"Stop all" → `cmd(instance_stop_all,{profile})`, close.

#### quick_play (layout + singleplayer/multiplayer/realms)

- Data: `cmd(profile_quick_play_list,{profile})` (UpdateType ProfileQuickPlay), split by type, each sorted lastPlayedTime desc.
- Tabs "Singleplayer" | "Multiplayer" | "Realms"; clicking navigates `/profiles/info/quick_play/{tab}?id=`. Active from path.
- Note under tabs when no worlds AND no servers: "Quick play needs Minecraft 1.20 (23w14a) or newer. Join a world once to add it here."
- **QuickPlayList**: list container; rows: QuickPlayIcon 40px (`cmd(profile_quick_play_icon)`, fallback Globe for singleplayer / Server otherwise), name, subtitle "Last played {de DATETIME_SHORT}" for singleplayer & realms, server address (`id`) for multiplayer.
  - Singleplayer actions: primary `Play` "Play" (launch with quickPlay); outline icon Star "Add to favorites"/"Remove from favorites" → `profile_favorites_set(profile, !fav, item)`; outline icon Trash2 (destructive color) "Delete world" → confirm.
  - Multiplayer: server status chip (`cmd(quick_play_server_status,{address})`, cached 60s per address): online → green "{online} / {max} online"; offline → secondary muted "Offline"; nothing while loading/failed. Then primary "Join".
  - Realms: primary "Join".
  - Empty states (dashed Empty): singleplayer "No worlds found" / "Join a singleplayer world to add it here."; multiplayer "No servers found" / "Join a server to add it here."; realms "No realms found" / "This account does not own or belong to a Realm."
  - Remove dialog: title "Remove Entry", desc "Are you sure you want to remove the quick play entry {name}?", destructive "Remove" → `cmd(profile_quick_play_remove,{profile, quickPlay})`.

#### content (layout + mods/resource-packs/shaders)

- `/profiles/info/content` → `/content/mods`. Tabs "Mods" | "Resource packs" | "Shaders" each with count pill (installed count, `cmd(content_installed,{profile,kind})`); click navigates.
- Page body = ContentList (see Shared).

#### instances

- Polls `instance_list` every 3000ms (also updates "now" for uptime).
- H2 "Running instances" + xs muted "Use Launch another above to start more. Pick a different account to play with two at once."
- InstanceRow (card variant) per instance of this profile (oldest first, numbered 1..n). Stop → DestroyDialog "Stop Instance" / "Are you sure you want to stop this instance?" / "Stop" → `cmd(instance_stop,{profile,id})`.
- Empty: "Not running" / "This profile has no running instance. Press Play above to start one."
- Footer xs: "Worlds are shared between instances, so only open a world in one of them at a time."

#### logs

- Data: `cmd(profile_runs_list,{profile})` (ISO timestamps, UpdateType ProfileLogs) sorted newest first; selected run default newest; `cmd(profile_logs,{profile,timestamp})`.
- Toolbar: search input "Search logs" (live, no debounce); dropdown button showing selected levels joined ", " or "All levels" (ChevronDown) with checkbox items INFO, WARN, ERROR, DEBUG (stays open on select, keeps canonical order); dropdown "Run: {de DATETIME_SHORT}" or "No runs" (max-h 320px) with radio items per run, separator, destructive `Trash` "Clear runs".
- LogWindow with `source` column, grows.
- Clear dialog: "Clear Logs" / "Are you sure you want to clear the logs of this profile?" / "Clear" → `cmd(profile_clear_logs)`.

#### settings (layout + general/minecraft/java)

- `/settings` → `/settings/general`. Max width 720px, scroll. Tabs "General" | "Minecraft" | "Java" (navigate on change).
- **General**:
  - ProfileIcon 64px + outline "Change icon" (hidden file input accept png/jpeg/jpg) → `cmd(profile_update_icon,{profile, icon bytes})`; error → "Failed to update profile icon".
  - "Name" input; on change (blur/enter): trimmed empty → toast "Profile name cannot be empty"; changed → `cmd(profile_update)`.
  - "Minecraft version" VersionSelect (`version_list(loader)`, current version prepended if missing). On change: for modded, keep loader_version if available for new version else first available; then update.
  - Modded only: "{Loader} loader version" VersionSelect (`loader_version_list`) → update.
  - Modded only: Alert TriangleAlert "Changing the version can break mods" / "Mods built for another version may fail to load."
  - Update failure toast: "Failed to update profile".
- **Minecraft**: SwitchTile "Use profile game settings" / "Override the global Minecraft settings for this profile only." (→ profile_update use_local_game, game = profile.game ?? global). GameSettings (disabled unless profile.game && use_local_game). Error "Failed to update profile".
- **Java**: SwitchTile "Use profile Java settings" / "Override the global Java settings for this profile only." JavaSettings with maxMem = settings.system_max_mem ?? 8192, label "{x.x} GB of {total} GB", hint "Most modded profiles run well with 4 to 6 GB.", disabled unless profile.jvm && use_local_jvm.

### /instances

- Data: `instance_list` polled every 3000ms.
- Header: H1 "Instances"; if any: muted "{n} game(s) running across {m} profile(s)." and destructive "Stop all" → confirm "Stop all instances" / "Are you sure you want to stop every running instance?" / "Stop all" → `cmd(instance_stop_all)`.
- Group per profile (groups ordered by first launch): header row: profile name link (→ `/profiles/info/instances?id=`), muted "{loader} {version}", green chip running_label, spacer, outline `Plus` "Launch another" → profile_launch. Below: list container of InstanceRow (row variant).
- Stop one: "Stop Instance" / `Are you sure you want to stop this instance of profile "{profile_name}"?` / "Stop".
- Empty: "No games running" / "Launch a profile to see its instances here." + ProfileExternalLink.

### /instances/info/logs `?id=`

- `/instances/info` → `/instances/info/logs?id=`. If instance list loaded and id missing → go `/instances`. Loading text "Loading...".
- Header: green dot 8px, H1 "{profile_name} console", Select (w-220px) of sibling instances of same profile labelled "Instance {i}[ · {account_name}]" → navigate to that id; destructive "Stop" → confirm "Stop Instance" / `Are you sure you want to stop the instance of profile "{profile_name}"?` / "Stop".
- Filters: search "Search console" (live); ToggleGroup (outline, single, cannot deselect) "All" | "Warnings" (WARN+ERROR) | "Errors" (ERROR).
- Logs: `cmd(instance_logs,{profile,id})`, refreshed on UpdateType InstanceLogs. LogWindow (no source column; message prefixed "location: "), min-h 480px, follow mode.
- Footer: Switch "Follow new output" bound to follow (auto-disables when user scrolls up, re-enables when scrolled to bottom).

### /discover/{mods,modpacks,resource-packs,shaders}

DiscoverPage with:

- mods: title "Mods", noun "mods", subtitle "Browse Modrinth and CurseForge and install straight into a profile."
- modpacks: "Modpacks", "modpacks", "Each modpack installs as its own profile, with every mod, pack and setting in place."
- resource-packs: "Resource packs", "resource packs", "Change textures, sounds and animations. Installed packs show up in the profile's content tab."
- shaders: "Shaders", "shaders", "Lighting, shadows and water. Shaders need Iris and Sodium, which Proton can add for you."
  (Behavior in Shared > DiscoverPage.)

### /discover/project `?kind=&source=&id=`

- Defaults kind "mod", source "modrinth". Data `cmd(content_project,{source,id})` (cached 60s); stores name for breadcrumb.
- Not found: Empty "Project not found" / "It may have been removed, or the source is not reachable right now." Nothing rendered while loading.
- Compatible profiles = profiles where profile_compat ok. Target (non-modpack) = remembered content target if compatible, else latest played compatible, else first compatible.
- Header: ProjectIcon 88px rounded-xl; name (xl semibold) + amber chip "Update available" if target has update; description (muted sm, max-w 560px); tags chips (categories ∪ loaders, tag_label).
- Right column (w-220px): TargetPicker (non-pack; only compatible profiles); button: update → "Update to {latest.versionNumber}"; installed → secondary disabled `Check` "Installed"; else "Install" (disabled if no target for non-pack). If installed: xs muted "Installed in {target}: {versionNumber|fileName}".
- Tabs "Description" | "Versions" | "Gallery" (lg: content scrolls independently; narrow wraps and page scrolls).
  - Description: sanitized HTML in `.prose-content` max-w 640px; clicking http links opens externally (openUrl).
  - Versions: (non-pack with target) Switch "Show all versions, not only those for {target}" (default off → `cmd(content_versions,{source,id,profile})`; on / modpack → all versions). Table columns: Version (medium) | Channel (tag_label) | Game version (sorted newest by manifest, short_list 2: "a, b, +n more") | Loaders | Published (luxon relative, e.g. "3 days ago") | Action (right): green chip "Installed" if this is installed version, else outline sm "Install" → install dialog preselecting that version. Empty "No versions" / filtered: "None made for {loader} {version}. Show all versions to see the rest." else "This project has no files to install."
  - Gallery: grid minmax(240px) of 16:9 images (lazy, rounded-lg, object-cover) with optional caption. Empty "No screenshots" / "This project has no gallery images."
- Aside (260px): card "Compatibility": rows "Game version" (short_list 3), "Loaders", "Runs on" ("Client"/"Server"/"Client and server"), "Source" link button ("Modrinth"/"CurseForge" + ExternalLink → openUrl project url). If newest compatible version has required deps: separator, "Needs", each dep name link → its project page (kind mod) + green "Installed" chip if in target's mods. Card "Also installed from this page" listing other profiles with this project: "{profile name}" → "{versionNumber}".
- Dialogs: modpack → ModpackInstallDialog(project); else InstallDialog(project, versionId).

### /settings (layout)

- SimpleSidebar (w-40, no icons): "Accounts", "Skins", "Capes", "Minecraft", "Java", "About". Content max-w 720px except Skins.

### /settings/accounts

- Header: H1 "Accounts", muted "The account marked Selected is used when you press Play."; primary "Add account" (Plus, or spinner while loading; disabled while loading).
- Add: offline → warning "You are currently offline, please reconnect to the internet to add an Account". Else loading toast "Waiting for Microsoft Login" (10 min) then `cmd(account_login)` (opens MS login). While waiting, `account-login-status` events update the same toast: Ms → "Logging in to Xbox", Xbox → "Logging in to Xbox Security", XboxSecurity → "Logging in to Minecraft", Mc → "Retrieving Minecraft Profile". Result: success "Successfully added Account" / error "Failed to add Account"; dismiss loading toast.
- List (`cmd(account_list)` map id → info|null, sorted by id): ring card rows: AccountImage (head 32px) or CircleHelp; name or "Login failed"; mono xs id; "Selected" badge if active else outline "Select" (only if info) → `cmd(account_set_active)` (error "Failed to switch Account"); ghost icon Trash2 "Remove account" → `cmd(account_remove)` → "Successfully removed Account" / "Failed to remove Account" (no confirm).
- Empty: "No accounts available".

### /settings/skins

- Layout: MainSkinViewer (left) + column. Header H1 "Skins", muted "Click a skin to wear it."; primary "Add skin" (spinner while uploading) → hidden file input accept png/jpeg → `cmd(account_add_skin,{skin bytes})` → "Successfully added Skin" / "Failed to add Skin".
- Grid minmax(120px) of MiniSkinViewer per `cmd(account_list_skins)` (selected = active skin url match). Change: offline warning "You are currently offline, please reconnect to the internet to change Skin"; `cmd(account_change_skin)` → "Successfully changed Skin" / "Failed to change Skin". Remove (hover trash, not on selected) → `cmd(account_remove_skin)` → "Successfully removed Skin" / "Failed to remove Skin".
- Empty: "No skins available".

### /settings/capes

- H1 "Capes", muted "Capes owned by {account name|'this account'}. Choose none to hide your cape."
- No active account: "No capes available". Skin texture loading: spinner block (h-37). Grid minmax(140px): first tile "No cape" (id REMOVE_CAPE, selected when no active cape), then each cape (spinner tile until texture loaded). Tiles: flipped (camera from back), height 130, no delete.
- Change: offline warning "You are currently offline, please reconnect to the internet to change Cape"; `cmd(account_change_cape,{id})` → "Successfully changed Cape" / "Failed to change Cape".

### /settings/minecraft

- H1 "Minecraft", muted "Defaults for every profile. Profiles can override them." Loading "Loading...".
- SwitchTile "Show snapshots" / "List snapshots and pre-releases when creating a profile."
- Tile "Download folders" / "Watched for files CurseForge only allows from its website." rows of path + ghost X "Remove {dir}"; outline sm `FolderPlus` "Add folder" → native directory picker titled "Add download folder" (dedupe). Error "Failed to save download folders".
- GameSettings (global). Errors: "Failed to save Minecraft settings"; no settings: "Failed to load settings".

### /settings/java

- H1 "Java", same subtitle as Minecraft. JavaSettings: maxMem = system_max_mem ?? 8192; label "{x.x} GB ({mb} MB)"; hint "Your system has {round(maxMem/1024)} GB. Leave at least 2 GB for the OS." Errors "Failed to save Java settings" / "Failed to load settings".

### /settings/about

- H1 "About". Ring card: "Proton" + xs muted "Version {app version}" + " · {v} available" or " · up to date" (after a check). Button: update known → primary "Install {v}" (runs update flow); else outline "Check for updates" (spinner, disabled while checking) → offline warning "You are currently offline, please reconnect to the internet to check for updates"; no update → toast.message "No updates available".
- SwitchTile "Update automatically" / "Install new versions when the launcher starts." (auto_update)
- SwitchTile "Hide window controls" / "Hide minimize, maximize and close in the title bar, e.g. for tiling window managers." (hide_window_controls). Save error "Failed to save settings".

---

## 4. Shared components & logic

### Data refresh (data-state)

- Each data-state lazily fetches on first subscriber and re-fetches whenever backend emits event `data-update` with payload = UpdateType: Accounts, AccountActive, AccountSkins, Versions, Profiles, ProfileLogs, ProfileQuickPlay, ProfileContent, ContentUpdates, Instances, InstanceLogs, Settings, Offline. Failed commands yield `undefined` (shown as empty/loading).
- Caches: content_search & content_project 60s per args; server status 60s per address; skin/cape textures by URL forever (retry on failure).

### Profile helpers

- compareProfiles: played profiles first by last_played desc; never-played by created_at desc.
- `card_meta`: [profile name (quick play) | "{n} mod(s)" (modded, known)] + "played {relative}" or "never played", joined " · ".
- running_label(n): n>1 "{n} running" else "Running".
- format_duration: "<1m", "{m}m", "{h}h {m}m".

### ProfileCard (props profile, quick_play?)

- Tile h-18 (72px) p-2.5 gap-3. Link (→ `/profiles/info/quick_play/{qp.type|singleplayer}?id=`) containing: 48px icon box (quick play icon if exists, else ProfileIcon fallback Box 28px), name/qp name (sm medium) + small filled Star if favorite, "{loader} {version}" (13px muted), line: green running chip (profiles only) + muted meta.
- Right button: running → destructive icon `Square` "Stop instances" → `instance_stop_all(profile)`; else primary icon `Play` "Play profile" → profile_launch(..., quick_play).

### profile_launch

- No active account → toast warning "No active account set". Else job `profile_launch` {profile, quickPlay} title "{name}: launch", ok "Launching profile {name}", err "Failed to launch profile {name}".

### ProfileIcon (id, class, classFallback)

- Avatar 48px rounded-md; `cmd(profile_get_icon)` base64 png, refreshed on UpdateType Profiles; fallback lucide `Box`.

### QuickPlayIcon (profileId, quickPlay, iconExists bindable, noFallback)

- `cmd(profile_quick_play_icon)`; fallback bg-muted with Globe (singleplayer) / Server (else) 20px; with noFallback hides when no icon.

### ProfileExternalLink

- Centered outline button "Profiles" + ExternalLink icon → `/profiles`.

### DuplicateDialog (open, profile)

- Title "Duplicate profile", desc "Copies the profile with its content, settings and worlds." Field "Name" (required, prefilled "{name} (copy)" on open). Footer outline "Cancel", primary "Duplicate" (disabled busy or blank, spinner). → `cmd(profile_duplicate,{profile,name})`; error → "Failed to duplicate profile {name}"; success: close, refresh list, navigate to `/profiles/info?id={new}` once it appears.

### DestroyDialog (open, title, description, btnText, onclick)

- Dialog with title, description, single destructive footer button. Escape/overlay closes.

### AccountSelector (sidebar bottom)

- Trigger h-12 full width hover accent/60: active account → AccountImage + name (sm) + id (xs muted) (text hidden below md) else CircleHelp + "No account active"; ChevronsUpDown icon (md+).
- Popover (w-200px): each account ghost button (head, name, id) → `cmd(account_set_active)` + close; null info rows: CircleHelp, "Login failed", id. No accounts: "No Accounts" + outline "Add One" (ExternalLink) → `/settings/accounts`.

### AccountImage (skin_url?)

- Skeleton 32px while loading; head image 32px rounded pixelated from `cmd(account_get_skin,{url}).head`; failure: CircleHelp with tooltip "Image could not be loaded".

### MainSkinViewer

- Card w-60 (240px) p-3.5: 3D skin canvas (skinview3d, 212×280, zoom 0.8, orbit enabled, zoom disabled) h-70 on bg-muted, spinner until ready. Native select "Animation": None, Idle (default), Walking, Running, Waving. Native select "Back equipment": None, Cape (default), Elytra (disabled when no active cape). Shows active skin + active cape.

### MiniSkinViewer (id, label, skin b64, selected, change_fn, cape?, flipped?, delete_disabled?, height=110)

- Whole tile is a button (input tile style, `border-primary` when selected, aria-pressed); 3D viewer no controls; flipped → camera z=-50 (back view). Spinner overlay while loading/changing. Click (not selected) → change_fn. Hover/focus reveals ghost icon Trash2 "Remove skin" top-right (unless delete_disabled or selected).

### InstanceRow (instance, number, now, card?, onstop)

- Green dot, "Instance {number}" (semibold; 15px in card), subtitle: [account (card: "Playing as {account}"), "running for {duration}"] " · ".
- Memory block (w-180px) when memory_used known: "Memory" + "{used} of {total} GB" (1 decimal, total = system_max_mem) or "{used} GB"; Progress percent.
- Outline "Console" → `/instances/info/logs?id=`; destructive "Stop" → onstop.
- Card variant: ring card px-4 py-3.5; row variant: list row.

### LogWindow (lines, source?, follow bindable=true, class)

- Ring box rounded-xl, min-h 420px, overflow both, mono 12.5px / line-height 20px, px-3.5 py-3, dark mix bg. Each entry nowrap: time (muted), level (w-42px medium, colored), then source mode: location[ (subLocation)] muted + message; else "location: message". Continuation lines (stack traces) shown raw `whitespace-pre`.
- Auto-scroll to bottom on new lines while follow; scrolling away from bottom sets follow false; at bottom (±1px) sets true.
- Empty: "No logs found for the current filter settings" (sans, sm, muted).

### logs.ts

- Line regex: `^\[(time)\] \[(location)/(LEVEL)\](?: \[(subLocation)\])?: (message)$`. Non-matching lines = continuation inheriting previous level.
- Level colors: INFO `text-green-500`, WARN `text-yellow-500`, ERROR `text-red-500`, DEBUG `text-blue-500`; others uncolored.
- LOG_LEVELS order: INFO, WARN, ERROR, DEBUG. Console modes: all = [], warn = [WARN, ERROR], err = [ERROR].
- Filter: level in set (empty = all) AND (empty query OR location/subLocation/message contains query, case-insensitive, trimmed).

### Date formatting

- Absolute dates: luxon `setLocale('de')` + `DATETIME_SHORT` (e.g. "16.10.2026, 14:30") — profile created/last played, log runs, quick play last played.
- Relative: luxon `toRelative()` default (English) e.g. "2 hours ago" — profile cards, project card "Updated …", versions table.
- Numbers: download counts `Intl.NumberFormat('en', {notation:'compact'})` ("2.1M"); download progress en-US grouping.

### Fuzzy search

- Only /profiles uses fuse.js (keys name 1 / version 0.5 / loader 0.5, threshold 0.4, extended search). All other searches are case-insensitive substring: TargetPicker & InstallDialog (profile name), ContentList (name or fileName), logs.

### VersionSelect (label, options, value bindable, disabled, onchange)

- pleiades Multiselect in single mode, full width: outline button showing selected value or "Select {label}"; popover with command search input placeholder "Search {label lowercase}...", empty "No {label} found", check mark on selected; selecting closes; cannot clear.

### Multiselect (filters on /profiles)

- Same component multi mode: button text = selected labels joined ", " or " {label}" (empty prefix); toggling items keeps popover open.

### FormImage / ImageInput

- Clickable avatar (default 80px, ring-accent ring-2 offset-2) previewing chosen image; fallback Upload icon + optional placeholder text; hidden file input accept png/jpeg/jpg; writes File into form field.

### SwitchTile (title, description, checked, disabled, onCheckedChange)

- Input-tile label (px-3.5 py-3): Switch left, title (medium) + description (xs muted). Whole tile clickable.

### SwitchTooltip

- Label with tooltip (side right) + Switch right. (Currently unused by routes.)

### GameSettings (settings, updateSettings, disabled)

- "Window size" label + right small label "Custom size (1.13+)" with small Switch (use_custom). Two number inputs (w-100px) width placeholder "854" × height placeholder "480"; disabled unless use_custom; save on change if >0 and changed.

### JavaSettings (settings, maxMem, memoryLabel, memoryHint, updateSettings, disabled)

- "Memory" + right label; Slider min 1024, max maxMem, step 256, live label while dragging, saves on commit; hint text.
- "Java arguments" input placeholder "-XX:+UseG1GC" (joined by spaces; on change split on whitespace).
- "Environment variables": list rows mono "KEY=value" sorted by key + ghost trash "Remove {key}"; add form: "KEY" input, "value" input, outline "Add" (Enter submits). Validation toasts: "Both key and value must be provided", `Environment variable key cannot contain "="`, `Environment variable "{key}" already exists`.

### TargetPicker (target?, profiles)

- Outline button h-10: ProfileIcon 24px, muted "Installing to", name, muted "{loader} {version}", ChevronDown; none: muted "No profile to install to"; disabled if no profiles. Popover (w-288px, align end): input "Search profiles", list max-h 320px of ghost buttons (secondary for current) with icon 32px, name, loader/version → sets remembered target (localStorage key `content-target`). Empty "No matching profiles."

### ProjectIcon (url?, name, class, classFallback)

- Avatar 40px bg-muted rounded-md; image or fallback Box (half size, muted).

### ProjectCard (project, action install|update|installed, oninstall)

- Ring card px-4 py-3.5: icon 72px (link), name link (base medium, hover underline) + xs muted "{Modrinth|CurseForge} · {author}", description (2 lines clamp), tags (categories, loaders for mod/modpack, runs_on), right column w-128px: `Download` icon + compact count, "Updated {relative}", button: installed → secondary disabled `Check` "Installed"; update → outline "Update"; install → primary "Install".
- Link → `/discover/project?kind={kind}&source={source}&id={encoded id}`.

### DiscoverPage (kind, title, subtitle, noun)

- Header: title + subtitle; right: TargetPicker (non-modpack) or primary chip "Creates a new profile" (modpack).
- Search input "Search {noun}" (debounce 300ms) + Select (w-176px) "Sort: {Relevance|Downloads|Updated|Newest}" (default Relevance).
- Left filter aside (232px, ring card, scroll):
  - mod/resourcepack: "Compatible only" switch (default on) with sub "No profile selected" / mod: "{loader} {version}, from {name}" / rp: "Made for {version}"; disabled w/o target; separator.
  - "Source": checkboxes Modrinth, CurseForge (only those in `cmd(content_sources)`; default both checked).
  - mod/modpack when not compatible-mode: "Loader": Fabric, Quilt, Forge, NeoForge.
  - not shader & not compatible-mode: "Game version" VersionSelect with "Any version" + vanilla versions.
  - Category groups from `cmd(content_categories,{kind})` grouped by header: "categories" → "Category" ("Look" for shaders) first; "features" → "Features", "performance impact" → "Performance cost", "resolutions" → "Resolution", others tag_label.
  - mod: "Runs on": Client / Server (mutually exclusive checkboxes).
- Results column (scroll): shader + target → Alert: ready (Iris YL57xq9U/455508 and Sodium AANobbMI/394468 installed) "{target} is ready for shaders" / "Iris Shaders and Sodium are already installed. On profiles without them, Proton adds both when you install a shader."; else "Shaders need Iris and Sodium" / "Proton adds both to {target} when you install a shader." Per backend warning: Alert TriangleAlert "Some results are missing" / {warning}. ProjectCards (action vs target's installed/updates; modpacks always install). "Load more" outline button when hits < total (page size 20, offset = hits). Empty (not loading): "No {noun} found" / "Try other search terms or fewer filters."; failure: "Search failed" / "Check your connection and try again." No skeletons; previous results stay while reloading.
- Query: compatible → `compatibleWith` = target id (backend fills loader/version), loaders/gameVersion omitted; environment only for mod/modpack; query trimmed; search waits for profile list.
- Install click → InstallDialog / ModpackInstallDialog.

### Compatibility rules (profile_compat)

- modpack: always ok. Game versions non-empty and not containing profile.version → "Not available for {version}". mod: loader in loaders (lowercase) ok; Quilt accepts fabric "Fabric version, runs on Quilt"; else "Not available for {Loader}". shader on Vanilla/Forge → "Shaders need Fabric, Quilt or NeoForge".

### InstallDialog (open, project, versionId?)

- Width min(520px). Title "Install {name}", desc "Choose which profile gets this {mod|pack}. Only compatible profiles can be selected."
- "Search profiles" input; radio tiles (max-h 288px): ProfileIcon 36px, name, note "{loader} {version}[ · modpack, edits may be replaced on update][ · {compat note}]"; incompatible tiles 50% opacity disabled. Empty "No profiles yet." / "No matching profiles." Default selection: user pick → remembered target → first compatible.
- "Version" VersionSelect options "{versionNumber} ({channel})" from `content_versions(source,id,profile)`; default = versionId → newest release → newest.
- Plan `cmd(content_plan_install,{profile, picks:[{source,projectId,versionId}]})`: loading "Checking dependencies" spinner; error Alert destructive: Incompatible → "Not compatible with this profile" / "This version can not run with the loader or Minecraft version of the profile."; other → "Could not resolve dependencies" / "Check your connection and try again."
- Plan view: box "Also needed" (uppercase xs) rows: already installed → green "Already installed"; dependency → secondary "Will install {versionNumber}". "Optional: a, b". Conflicts Alert destructive "Conflicts with installed content" with "{name}: {reason}" lines. Manual files: "CurseForge only allows these files to be downloaded from its website. Download each one, it is picked up automatically." + ManualDownloads.
- Footer: outline "Cancel", primary `Download` "Install to {profile name|'profile'}" (enabled when plan, no conflicts, all manual found). Install: closes, remembers target, adopts manual files (`cmd(content_adopt_file)`; failure toast "Failed to add {file} to {profile}"), then job `content_install` title "{profile}: install", ok "Installed {item name | n items} in {profile}", err "Failed to install content in {profile}"; if nothing left to download: success "Installed {project} in {profile}".

### ModpackInstallDialog (open, project?, plan?, profile?)

- Width min(540px). Install mode: title "Install {pack name}", desc "This creates a new profile. Your other profiles are not touched." Change mode (profile): title "Change version of {profile}", desc "Pack files are replaced with the chosen version. Content you added stays, edited configs may be overwritten."
- Row: ProjectIcon 64px; plan → "{loader} · Minecraft {mc}" + chips "{n} mod(s)", "{n} resource pack(s)", "{n} shader(s)" (non-zero); error: InvalidPack "This pack version can not be read." else "Failed to load the pack, check your connection."; loading "Loading pack" spinner.
- Fields: (install mode) "Profile name" input (default plan name); "Pack version" (+ " · installed {v}" in change mode) VersionSelect from `content_versions(source,id)`; plan = given plan or `cmd(modpack_plan,{source:{type:'remote',...versionId}})`.
- Memory alert (install mode, pack recommends memory): fits (rec + 2048 ≤ system) → Info "Memory is set up for you" / "The pack asks for {x.x GB}. Your system has {y.y GB}, so nothing needs changing."; else yellow TriangleAlert "Not enough memory for this pack" / "The pack asks for {x}, but your system only has {y}. The game may run slowly or crash."
- Manual files Alert "{n} file(s) need a manual download" / "CurseForge only allows these from its website. Download each one, it is picked up automatically." + ManualDownloads.
- Checkbox "Start the game when the download finishes".
- Footer: "Cancel"; change mode "Change version" (disabled if no plan, same version as installed, or manual pending) → job `modpack_update` title "{name}: update pack", ok "Updated {name} to {version}", err "Failed to update {name}"; install mode "Create profile and install" (disabled if no plan, blank name, manual pending) → job `modpack_install` title "{name}: install", ok "Installed modpack {name}", err "Failed to install modpack {name}". Then launch if checkbox.

### ManualDownloads (files, found bindable)

- Text: "Watching {dirs}. Change the folders in Settings, Minecraft." or "No download folders set, add one in Settings, Minecraft."
- List (max-h 256px): file name; found → green `Check` "Found"; else spinner "Waiting" + outline sm `Download` "Download" (opens downloadUrl in browser). Polls `cmd(content_find_manual,{files})` every 2s while any missing.

### ContentList (kind mod|resourcepack|shader, profile)

- Toolbar: search "Search installed {word}s"; status Select (not shaders, w-144px): "All status" | "Enabled" | "Disabled" | "Updates"; outline "Check for updates" (RefreshCw / spinner) → `cmd(content_check_updates,{profile})` then refresh updates; primary `Plus` "Add {word}s" → remember target, go `/discover/{route}`.
- Update Alert when updates: "{n} {word}(s) can be updated" / "Newer versions compatible with {loader} {version} are available." action sm "Update all" → job `content_update` (fileNames null) title "{profile}: update", ok "Updated content in {profile}", err "Failed to update content in {profile}".
- Selection bar when ≥1 selected: "{n} selected", outline "Update" (if any selected has update), "Enable", "Disable" (not shaders), outline destructive-text `Trash` "Remove".
- List header (h-9 bg-muted/40): select-all checkbox (indeterminate when partial) over visible rows, "{n} {word}(s)", "Version" (md+), "Source" (md+), "Enabled"/"Active".
- Rows (min-h 56px): resource packs: drag handle `GripVertical` (enabled ones only; drag onto another enabled row reorders → `cmd(content_set_order)`, dragged row 50% opacity); checkbox; ProjectIcon 40px; name or fileName + chips "Dependency" (dependencyOf non-empty), "From pack"; fileName (xs muted); version (or "-") + yellow "Update: {v}"; source chip "Modrinth"/"CurseForge"/"Manual"; right: xs outline "Update" if update, Switch enable (`cmd(content_set_enabled)`) or radio (shaders, `cmd(content_set_active_shader)`); ghost Ellipsis "More actions" menu: `ExternalLink` "Open project page" (if source), `FolderOpen` "Show in folder" (`profile_open_path`), destructive `Trash` "Remove".
- Shaders list ends with row "None (shaders off)" (dashed placeholder icon) radio value "" → active shader null.
- Sort: resource packs enabled by order then disabled by name; others by name (case-insensitive).
- Empty: "No {word}s installed yet" or "No {word}s match the filters".
- Remove: no confirm unless dependents or manual files; then DestroyDialog title "Remove {word|words}", desc "Still needed by {names}, which may stop working." and/or "{files} was/were added manually and cannot be re-downloaded.", button "Remove" → `cmd(content_remove,{profile,kind,fileNames})`.
- Errors toast: Running → "Stop the game first"; others "Something went wrong, please try again".
- ContentDropZone below: dashed box h-16 "Drop {.jar|.zip} files here to add a {word} manually" (Upload icon); highlights (primary border/bg) during window drag; drop → wrong ext warning "Only {ext} files can be added here"; each accepted → `cmd(content_add_file,{profile,path})`.
- Word: mod "mod", resourcepack "resource pack", shader "shader".

### SimpleSidebar (items {href,title,icon?,match?,badge?}, class)

- Vertical links h-8, px-2.5, gap-2, sm medium; active when pathname startsWith(match ?? href path) and search equals href search; active bg-muted crossfade indicator (250ms cubicInOut); amber count badge right when badge > 0. Navigation does not scroll to top.

### tag_label / helpers

- tag_label: replace "-" with space, capitalize first letter ("world-generation" → "World generation").
- short_list(list, n=3): "a, b, c, +k more".
- runs_on: client/server side not "unsupported" → "Client and server" / "Client" / "Server".

---

## 5. Reference screenshots (`fixtures/reference/*.png`)

- Run `just reference [slug-filter]` (Hyprland only). It runs `fixtures/scripts/reference.sh`, which builds `target/release/proton` if missing (`npx tauri build --no-bundle`; a release build is required because debug builds redirect the persisted URL to the vite dev server). For every route it copies `fixtures/data` to a temp `XDG_DATA_HOME`, writes `settings.url = tauri://localhost<route>`, and starts the app under `setsid dbus-run-session` (its own process group, plus a private bus so the single-instance plugin ignores a Proton that is already running). It then floats the window, resizes it to 1300x800, moves it to 40,40, waits `WAIT` seconds (default 11 so the 10s offline toast is gone, +6 for online routes) and captures the window with `grim -s 1`.
- Every route except discover runs in `unshare -n --map-current-user` (no network), so the app is deterministically **offline**: the header shows the "Offline" badge, there are no update checks, and content icons fall back to placeholders. Discover routes and `discover-project-sodium` (`/discover/project?id=AANobbMI&kind=mod&source=modrinth`) run online and show live Modrinth/CurseForge data, which changes over time.
- Captured (59): home, profiles, instances, instances-info-logs, settings-{accounts,skins,capes,minecraft,java,about}, discover-{mods,modpacks,resource-packs,shaders}, discover-project-sodium, plus `profile-<vanilla|fabric|forge|modpack>-<tab>` for every tab: quick_play-{singleplayer,multiplayer,realms}, content-{mods,resource-packs,shaders}, instances, logs, settings-{general,java,minecraft}.
- Gaps:
  - The profile create dialog (tabs Create / From modpack / Import file) cannot be opened by URL and there is no input injection, so it has no screenshot. Its spec is in §3 /profiles.
  - No running instance: instances are live game processes. `/instances` shows the "No games running" empty state, and `/instances/info/logs?id=none` redirects to `/instances` (its screenshot equals instances.png). The profile Instances tabs show "Not running".
  - Values that are not deterministic: relative times ("played 15 hours ago") depend on the current date, memory sliders show the host's RAM (`MaxMem` overrides `system_max_mem`), and discover results are live.
  - While offline the Fabric loader version select is empty (the loader version list does not resolve offline). Forge shows its version.

### Fixture data (`fixtures/data/io.profidev.proton`, regenerate with `fixtures/scripts/gen_data.py [real-data-dir]`)

- `store.json`:
  - 2 fake accounts, Steve_Fixture (active, 4 capes, Vanilla active) and Alex_Fixture (1 cape). Tokens are placeholder strings, and token expiry is 2099, so no refresh is attempted.
  - Skin and cape caches whose URLs match the account textures, so nothing is downloaded.
  - 4 profiles, `content_updates_checked` set to 2099, and settings with mem 4096 and window 854x480.
- Profiles:
  - **Vanilla Survival** (11111111-…, 1.21.8): favorite. Quick play: 2 singleplayer worlds with `saves/` dirs and icons ("New World" favorite and history, "Creative Test" history), 2 servers ("Hypixel" favorite), 1 realm. 3 resource packs (2 enabled through `options.txt`, 1 manual). 2 log runs, one ending in an ERROR stack trace.
  - **Fabric Performance** (22222222-…, Fabric 0.17.2): 7 mods (Debugify disabled as `.jar.disabled`, `local-tweaks-1.0.jar` manual, Mod Menu beta), 2 shaders with Complementary active (`config/iris.properties`), profile JVM override (6 GB, `-XX:+UseG1GC`), 1 log run.
  - **Forge Classic** (33333333-…, Forge 47.3.0): not downloaded, never played, 3 CurseForge mods.
  - **Fabulously Optimized** (44444444-…): modpack ref modrinth `1KVo5zza`, favorite, 4 mods plus 1 resource pack (all `fromPack`), 1 log run.
- Content files are tiny placeholders. `proton-content.json` entries match by file name, so no hashing or lookup happens.
- Public data was copied from a real install so the app starts offline: the Mojang version manifest and Java manifest, the fabric/quilt/forge/neoforge index JSONs, and the skin/cape PNGs.
