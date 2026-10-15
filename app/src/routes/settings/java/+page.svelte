<script lang="ts">
  import { toast } from 'svelte-sonner';
  import { settings_get, settings_set } from '$lib/tauri/settings.svelte';
  import type { JvmSettings } from '$lib/tauri/profile.svelte';
  import JavaSettings from '$lib/components/settings/JavaSettings.svelte';
  import { memory_with_mb } from '$lib/components/settings/settings';

  let settings = $derived(settings_get.value);
  let maxMem = $derived(settings?.system_max_mem ?? 8192);

  const saveSettings = async (jvm_settings: JvmSettings) => {
    if (!settings) {
      toast.error('Failed to load settings');
      return;
    }

    if (
      await settings_set({
        ...settings,
        minecraft: { ...settings.minecraft, jvm_settings }
      })
    ) {
      toast.error('Failed to save Java settings');
    }
  };
</script>

<div class="flex max-w-[720px] min-w-0 flex-1 flex-col gap-4">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Java</h1>
    <p class="text-muted-foreground mt-0.5">
      Defaults for every profile. Profiles can override them.
    </p>
  </div>
  {#if settings}
    <JavaSettings
      settings={settings.minecraft.jvm_settings}
      {maxMem}
      memoryLabel={memory_with_mb}
      memoryHint="Your system has {Math.round(
        maxMem / 1024
      )} GB. Leave at least 2 GB for the OS."
      updateSettings={saveSettings}
    />
  {:else}
    <p>Loading...</p>
  {/if}
</div>
