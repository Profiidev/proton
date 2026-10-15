<script lang="ts">
  import { toast } from 'svelte-sonner';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Slider } from '@profidev/pleiades/components/ui/slider';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import type { JvmSettings } from '$lib/tauri/profile.svelte';
  import { Trash2 } from '@lucide/svelte';
  import {
    add_env_var,
    parse_args,
    remove_env_var,
    sorted_env
  } from './settings';

  interface Props {
    settings: JvmSettings;
    maxMem: number;
    memoryLabel: (mb: number) => string;
    memoryHint: string;
    updateSettings: (settings: JvmSettings) => Promise<void>;
    disabled?: boolean;
  }

  let {
    settings,
    maxMem,
    memoryLabel,
    memoryHint,
    updateSettings,
    disabled
  }: Props = $props();

  // live value while dragging, saved on commit
  let mem = $derived(settings.mem_max);
  let newKey = $state('');
  let newValue = $state('');

  const save = (new_settings: Partial<JvmSettings>) =>
    updateSettings({ ...settings, ...new_settings });

  const addEnv = () => {
    const env = add_env_var(settings.env_vars, newKey, newValue);
    if (typeof env === 'string') {
      toast.error(env);
      return;
    }
    save({ env_vars: env });
    newKey = '';
    newValue = '';
  };
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex items-center justify-between gap-2">
    <Label>Memory</Label>
    <span class="text-xs">{memoryLabel(mem)}</span>
  </div>
  <Slider
    type="single"
    bind:value={mem}
    min={1024}
    max={maxMem}
    step={256}
    onValueCommit={(value) => {
      if (value !== settings.mem_max) save({ mem_max: value });
    }}
    {disabled}
  />
  <span class="text-muted-foreground text-xs">{memoryHint}</span>
</div>
<div class="flex flex-col gap-1.5">
  <Label for="jvm-args">Java arguments</Label>
  <Input
    id="jvm-args"
    placeholder="-XX:+UseG1GC"
    value={settings.args.join(' ')}
    onchange={(e) =>
      save({ args: parse_args((e.target as HTMLInputElement).value) })}
    {disabled}
  />
</div>
<div class="flex flex-col gap-1.5">
  <Label for="env-key">Environment variables</Label>
  {#if Object.keys(settings.env_vars).length > 0}
    <div class="bg-card/50 flex flex-col overflow-hidden rounded-xl border">
      {#each sorted_env(settings.env_vars) as [key, value] (key)}
        <div
          class="hover:bg-accent/40 flex items-center gap-3 border-b px-3 py-2.5 last:border-0"
        >
          <span class="flex-1 font-mono text-sm break-all">{key}={value}</span>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Remove {key}"
            onclick={() =>
              save({ env_vars: remove_env_var(settings.env_vars, key) })}
            {disabled}
          >
            <Trash2 class="text-destructive" />
          </Button>
        </div>
      {/each}
    </div>
  {/if}
  <form
    class="flex gap-2"
    onsubmit={(e) => {
      e.preventDefault();
      addEnv();
    }}
  >
    <Input
      id="env-key"
      placeholder="KEY"
      aria-label="Key"
      bind:value={newKey}
      {disabled}
    />
    <Input
      placeholder="value"
      aria-label="Value"
      bind:value={newValue}
      {disabled}
    />
    <Button type="submit" variant="outline" {disabled}>Add</Button>
  </form>
</div>
