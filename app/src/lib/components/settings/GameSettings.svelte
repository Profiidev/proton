<script lang="ts">
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';
  import { Input, Label } from 'positron-components/components/ui';
  import { cn } from 'positron-components/utils';
  import type { GameSettings } from '$lib/tauri/profile.svelte';

  interface Props {
    settings?: GameSettings;
    updateSettings: (settings: GameSettings) => Promise<void>;
    disabled?: boolean;
  }

  let { settings, updateSettings, disabled }: Props = $props();

  const saveGameSettings = async (new_settings: Partial<GameSettings>) => {
    await updateSettings({
      ...settings!,
      ...new_settings
    });
  };
</script>

{#if settings}
  <div class="flex flex-col gap-2">
    <SwitchTooltip
      id="use-custom-window-size"
      label="Use custom Window Size (1.13+)"
      tooltip="Changes the size of the Minecraft window to a custom resolution."
      checked={settings.use_custom ?? false}
      onCheckedChange={(value) => {
        saveGameSettings({
          use_custom: value
        });
      }}
      {disabled}
    />
    <div class="ml-6 flex items-center gap-2">
      <Label
        for="custom-window-size"
        class={cn(
          'whitespace-nowrap',
          (!settings.use_custom || disabled) && 'text-muted-foreground'
        )}>Custom Window Size</Label
      >
      <Input
        id="custom-window-size"
        type="number"
        placeholder="e.g 854"
        class="ml-auto max-w-20"
        value={settings.width ?? ''}
        disabled={!settings.use_custom || disabled}
        oninput={(e) => {
          const value = (e.target as HTMLInputElement)?.value;
          if (value && !isNaN(Number(value))) {
            saveGameSettings({
              width: Number(value)
            });
          }
        }}
      />
      <span
        class={!settings.use_custom || disabled ? 'text-muted-foreground' : ''}
        >x</span
      >
      <Input
        id="custom-window-height"
        type="number"
        placeholder="e.g 480"
        class="max-w-20"
        value={settings.height ?? ''}
        disabled={!settings.use_custom || disabled}
        oninput={(e) => {
          const value = (e.target as HTMLInputElement)?.value;
          if (value && !isNaN(Number(value))) {
            saveGameSettings({
              height: Number(value)
            });
          }
        }}
      />
    </div>
  </div>
{:else}
  <p>Loading...</p>
{/if}
