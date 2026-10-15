<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    settings_get,
    settings_set,
    type MinecraftSettings
  } from '$lib/tauri/settings.svelte';
  import GameSettings from '$lib/components/settings/GameSettings.svelte';
  import SwitchTile from '$lib/components/settings/SwitchTile.svelte';

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
    <GameSettings
      settings={settings.minecraft.game_settings}
      updateSettings={(game_settings) => saveSettings({ game_settings })}
    />
  {:else}
    <p>Loading...</p>
  {/if}
</div>
