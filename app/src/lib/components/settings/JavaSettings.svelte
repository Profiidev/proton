<script lang="ts">
  import { toast } from 'svelte-sonner';
  import {
    Button,
    Input,
    Label,
    ScrollArea,
    Slider
  } from 'positron-components/components/ui';
  import type { JvmSettings } from '$lib/tauri/profile.svelte';
  import { Plus, Trash } from '@lucide/svelte';

  interface Props {
    settings?: JvmSettings;
    maxMem: number;
    updateSettings: (settings: JvmSettings) => Promise<void>;
  }

  let { settings, updateSettings, maxMem }: Props = $props();

  let newKey = $state('');
  let newValue = $state('');

  const saveJavaSettings = async (new_settings: Partial<JvmSettings>) => {
    await updateSettings({
      ...settings!,
      ...new_settings
    });
  };

  const checkKey = (key: string) => {
    if (!settings) return;
    if (!key) {
      toast.error('Environment variable key cannot be empty');
      return;
    }
    if (Object.keys(settings.env_vars).includes(key)) {
      toast.error(`Environment variable "${key}" already exists`);
      return;
    }
    return true;
  };
</script>

<div class="mt-4 mr-4 mb-4 flex min-h-0 flex-grow-1 flex-col gap-4">
  {#if settings}
    <div class="flex items-center gap-2">
      <Label for="jvm-memory-slider" class="whitespace-nowrap"
        >Java Memory Limit</Label
      >
      <Input
        id="jvm-memory-slider"
        type="number"
        class="ml-auto max-w-24 text-right"
        value={settings.mem_max ?? 1024}
        min={512}
        max={maxMem}
        step={64}
        onfocusout={(e) => {
          const value = parseInt((e.target as HTMLInputElement).value, 10);
          if (value !== settings.mem_max) {
            saveJavaSettings({ mem_max: value });
          }
        }}
      />
      <p class="text-sm whitespace-nowrap">MB</p>
      <Slider
        type="single"
        id="jvm-memory-slider"
        class="max-w-64"
        value={settings.mem_max ?? 1024}
        min={512}
        step={64}
        max={maxMem}
        onValueChange={(value) => {
          if (value === settings.mem_max) return;
          saveJavaSettings({ mem_max: value });
        }}
      />
    </div>
    <Label for="jvm-args" class="whitespace-nowrap">Java Arguments</Label>
    <Input
      id="jvm-args"
      type="text"
      placeholder="e.g -XX:+UseG1GC -XX:MaxGCPauseMillis=50"
      value={settings.args.join(' ') ?? ''}
      oninput={(e) => {
        const value = (e.target as HTMLInputElement).value
          .trim()
          .split(/\s+/)
          .filter(Boolean);
        if (value !== settings.args) {
          saveJavaSettings({ args: value });
        }
      }}
    />
    <Label for="jvm-env" class="whitespace-nowrap">Environment Variables</Label>
    {#if Object.keys(settings.env_vars).length === 0}
      <p class="text-muted-foreground text-sm">No environment variables set</p>
    {:else}
      <ScrollArea.ScrollArea class="min-h-0">
        <div class="flex flex-col gap-2">
          {#each Object.entries(settings.env_vars).sort( ([keyA], [keyB]) => keyA.localeCompare(keyB) ) as [key, value]}
            <div class="flex items-center gap-2">
              <Input
                type="text"
                placeholder="e.g PATH"
                value={key}
                onfocusout={(e) => {
                  const newKey = (e.target as HTMLInputElement).value.trim();
                  if (!checkKey(newKey)) return;
                  if (newKey !== key) {
                    const newEnv = {
                      ...settings.env_vars
                    };
                    delete newEnv[key];
                    newEnv[newKey] = value;
                    saveJavaSettings({ env_vars: newEnv });
                  }
                }}
              />
              <Input
                type="text"
                placeholder="e.g /usr/bin/java"
                {value}
                onfocusout={(e) => {
                  const newValue = (e.target as HTMLInputElement).value.trim();
                  if (!newValue) {
                    toast.error('Environment variable value cannot be empty');
                    return;
                  }
                  if (newValue !== value) {
                    const newEnv = {
                      ...settings.env_vars
                    };
                    newEnv[key] = newValue;
                    saveJavaSettings({ env_vars: newEnv });
                  }
                }}
              />
              <Button
                size="icon"
                variant="destructive"
                onclick={() => {
                  const newEnv = {
                    ...settings.env_vars
                  };
                  delete newEnv[key];
                  saveJavaSettings({ env_vars: newEnv });
                }}
              >
                <Trash />
              </Button>
            </div>
          {/each}
        </div>
      </ScrollArea.ScrollArea>
    {/if}
    <div class="flex items-center gap-2">
      <Input type="text" placeholder="e.g NEW_VAR" bind:value={newKey} />
      <Input type="text" placeholder="e.g 123" bind:value={newValue} />
      <Button
        size="icon"
        onclick={() => {
          if (!newKey.trim() || !newValue.trim()) {
            toast.error('Both key and value must be provided');
            return;
          }
          if (!checkKey(newKey)) return;
          const newEnv = { ...settings.env_vars };
          newEnv[newKey.trim()] = newValue.trim();
          saveJavaSettings({ env_vars: newEnv });
          newKey = '';
          newValue = '';
        }}
      >
        <Plus />
      </Button>
    </div>
  {:else}
    <p>Loading...</p>
  {/if}
</div>
