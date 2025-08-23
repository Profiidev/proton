<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    settings_get,
    settings_set,
    type MinecraftSettings
  } from '$lib/tauri/settings.svelte';
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';
  import GameSettings from '$lib/components/settings/GameSettings.svelte';
  import type { GameSettings as GameSettingsType } from '$lib/tauri/profile.svelte';

  let settings = $derived(settings_get.value);

  const saveSettings = async (new_settings: Partial<MinecraftSettings>) => {
    if (!settings) {
      toast.error('Failed to load settings');
      return;
    }

    if (
      await settings_set({
        ...settings,
        minecraft: {
          ...settings.minecraft,
          ...new_settings
        }
      })
    ) {
      toast.error('Failed to save Minecraft settings');
    }
  };

  const saveGameSettings = async (new_settings: Partial<GameSettingsType>) => {
    await saveSettings({
      game_settings: {
        ...settings!.minecraft.game_settings,
        ...new_settings
      }
    });
  };
</script>

<div class="mt-2 ml-4 flex-1">
  <div class="flex items-center">
    <p class="text-xl">Minecraft</p>
  </div>
  <div class="mt-4 mr-4 flex flex-col gap-4">
    {#if settings}
      <SwitchTooltip
        id="show-snapshots"
        label="Show Snapshots"
        tooltip="Adds all snapshot/unstable versions to the list of selectable versions."
        checked={settings.minecraft.show_snapshots ?? false}
        onCheckedChange={(value) => {
          saveSettings({ show_snapshots: value });
        }}
      />
      <GameSettings
        settings={settings.minecraft.game_settings}
        updateSettings={saveGameSettings}
      />
    {:else}
      <p>Loading...</p>
    {/if}
  </div>
</div>
