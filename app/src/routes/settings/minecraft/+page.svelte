<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    settings_get,
    settings_set,
    type MinecraftSettings
  } from '$lib/tauri/settings.svelte';
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';

  let settings = $derived(settings_get.value);

  const saveSettings = async (new_settings: Partial<MinecraftSettings>) => {
    if (!settings) {
      toast.error('Failed to load settings');
      return;
    }

    if (
      !(await settings_set({
        ...settings,
        minecraft: {
          ...settings.minecraft,
          ...new_settings
        }
      }))
    ) {
      toast.success('Minecraft settings saved successfully');
    } else {
      toast.error('Failed to save Minecraft settings');
    }
  };
</script>

<div class="mt-2 ml-4 flex-1">
  <div class="flex items-center">
    <p class="text-xl">Minecraft</p>
  </div>
  <div class="mt-2 flex flex-col gap-2">
    {#if settings}
      <SwitchTooltip
        label="Show Snapshots"
        tooltip="Adds all snapshot/unstable versions to the list of selectable versions."
        checked={settings.minecraft.show_snapshots ?? false}
        onCheckedChange={(value) => {
          saveSettings({ show_snapshots: value });
        }}
      />
      <SwitchTooltip
        label="Custom Window Size (1.13+)"
        tooltip="Changes the size of the Minecraft window to a custom resolution."
        checked={settings.minecraft.custom_window_size ?? false}
        onCheckedChange={(value) => {
          saveSettings({ custom_window_size: value });
        }}
      />
    {:else}
      <p>Loading...</p>
    {/if}
  </div>
</div>
