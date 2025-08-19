<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    settings_get,
    settings_set,
    type MinecraftSettings
  } from '$lib/tauri/settings.svelte';
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';
  import { Input, Label } from 'positron-components/components/ui';
  import { cn } from 'positron-components';

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
      <div class="flex flex-col gap-2">
        <SwitchTooltip
          id="use-custom-window-size"
          label="Use custom Window Size (1.13+)"
          tooltip="Changes the size of the Minecraft window to a custom resolution."
          checked={settings.minecraft.custom_window_size ?? false}
          onCheckedChange={(value) => {
            saveSettings({ custom_window_size: value });
          }}
        />
        <div class="ml-6 flex items-center gap-2">
          <Label
            for="custom-window-size"
            class={cn(
              'whitespace-nowrap',
              !settings.minecraft.custom_window_size && 'text-muted-foreground'
            )}>Custom Window Size</Label
          >
          <Input
            id="custom-window-size"
            type="number"
            placeholder="e.g 854"
            class="ml-auto max-w-20"
            value={settings.minecraft.custom_window_width ?? ''}
            disabled={!settings.minecraft.custom_window_size}
            oninput={(e) => {
              const value = (e.target as HTMLInputElement)?.value;
              if (value && !isNaN(Number(value))) {
                saveSettings({ custom_window_width: Number(value) });
              }
            }}
          />
          <span
            class={!settings.minecraft.custom_window_size
              ? 'text-muted-foreground'
              : ''}>x</span
          >
          <Input
            id="custom-window-height"
            type="number"
            placeholder="e.g 480"
            class="max-w-20"
            value={settings.minecraft.custom_window_height ?? ''}
            disabled={!settings.minecraft.custom_window_size}
            oninput={(e) => {
              const value = (e.target as HTMLInputElement)?.value;
              if (value && !isNaN(Number(value))) {
                saveSettings({ custom_window_height: Number(value) });
              }
            }}
          />
        </div>
      </div>
    {:else}
      <p>Loading...</p>
    {/if}
  </div>
</div>
