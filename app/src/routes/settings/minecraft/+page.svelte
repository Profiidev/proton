<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    settings_get,
    settings_set,
    type MinecraftSettings
  } from '$lib/tauri/settings.svelte';
  import GameSettings from '$lib/components/settings/GameSettings.svelte';
  import SwitchTile from '$lib/components/settings/SwitchTile.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { FolderPlus, X } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';

  let settings = $derived(settings_get.value);

  const saveSettings = async (new_settings: Partial<MinecraftSettings>) => {
    if (!settings) {
      toast.error('Failed to load settings');
      return;
    }

    if (
      await settings_set({
        ...settings,
        minecraft: { ...settings.minecraft, ...new_settings }
      })
    ) {
      toast.error('Failed to save Minecraft settings');
    }
  };

  const saveDirs = async (download_dirs: string[]) => {
    if (!settings) return;
    if (await settings_set({ ...settings, download_dirs })) {
      toast.error('Failed to save download folders');
    }
  };

  const addDir = async () => {
    const dir = await open({ directory: true, title: 'Add download folder' });
    const dirs = settings?.download_dirs ?? [];
    if (dir && !dirs.includes(dir)) await saveDirs([...dirs, dir]);
  };
</script>

<div class="flex max-w-[720px] min-w-0 flex-1 flex-col gap-4">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Minecraft</h1>
    <p class="text-muted-foreground mt-0.5">
      Defaults for every profile. Profiles can override them.
    </p>
  </div>
  {#if settings}
    <SwitchTile
      title="Show snapshots"
      description="List snapshots and pre-releases when creating a profile."
      checked={settings.minecraft.show_snapshots ?? false}
      onCheckedChange={(show_snapshots) => saveSettings({ show_snapshots })}
    />
    <div
      class="border-input bg-input/30 flex flex-col gap-2 rounded-lg border px-3.5 py-3"
    >
      <span class="flex flex-col">
        <span class="font-medium">Download folders</span>
        <span class="text-muted-foreground text-xs">
          Watched for files CurseForge only allows from its website.
        </span>
      </span>
      {#each settings.download_dirs ?? [] as dir (dir)}
        <div class="flex items-center gap-2">
          <span class="min-w-0 flex-1 truncate text-sm">{dir}</span>
          <Button
            size="icon"
            variant="ghost"
            aria-label="Remove {dir}"
            onclick={() =>
              saveDirs(
                (settings?.download_dirs ?? []).filter((d) => d !== dir)
              )}
          >
            <X />
          </Button>
        </div>
      {/each}
      <Button size="sm" variant="outline" class="self-start" onclick={addDir}>
        <FolderPlus />
        Add folder
      </Button>
    </div>
    <GameSettings
      settings={settings.minecraft.game_settings}
      updateSettings={(game_settings) => saveSettings({ game_settings })}
    />
  {:else}
    <p>Loading...</p>
  {/if}
</div>
