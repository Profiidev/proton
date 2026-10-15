<script lang="ts">
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import type { GameSettings } from '$lib/tauri/profile.svelte';

  interface Props {
    settings: GameSettings;
    updateSettings: (settings: GameSettings) => Promise<void>;
    disabled?: boolean;
  }

  let { settings, updateSettings, disabled }: Props = $props();

  const save = (new_settings: Partial<GameSettings>) =>
    updateSettings({ ...settings, ...new_settings });

  const saveSize = (key: 'width' | 'height', e: Event) => {
    const value = Number((e.target as HTMLInputElement).value);
    if (value > 0 && value !== settings[key]) {
      save({ [key]: value });
    }
  };
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex items-center justify-between gap-2">
    <Label for="window-width">Window size</Label>
    <Label class="text-muted-foreground text-xs font-normal">
      Custom size (1.13+)
      <Switch
        size="sm"
        checked={settings.use_custom}
        onCheckedChange={(use_custom) => save({ use_custom })}
        {disabled}
      />
    </Label>
  </div>
  <div class="flex items-center gap-2">
    <Input
      id="window-width"
      type="number"
      aria-label="Width"
      placeholder="854"
      class="w-25"
      value={settings.width}
      disabled={disabled || !settings.use_custom}
      onchange={(e) => saveSize('width', e)}
    />
    <span class="text-muted-foreground">×</span>
    <Input
      type="number"
      aria-label="Height"
      placeholder="480"
      class="w-25"
      value={settings.height}
      disabled={disabled || !settings.use_custom}
      onchange={(e) => saveSize('height', e)}
    />
  </div>
</div>
