<script lang="ts">
  import { toast } from 'svelte-sonner';
  import { settings_get, settings_set } from '$lib/tauri/settings.svelte';
  import type { JvmSettings } from '$lib/tauri/profile.svelte';
  import JavaSettings from '$lib/components/settings/JavaSettings.svelte';

  let settings = $derived(settings_get.value);

  const saveSettings = async (new_settings: JvmSettings) => {
    if (!settings) {
      toast.error('Failed to load settings');
      return;
    }

    if (
      await settings_set({
        ...settings,
        minecraft: {
          ...settings.minecraft,
          jvm_settings: {
            ...settings.minecraft.jvm_settings,
            ...new_settings
          }
        }
      })
    ) {
      toast.error('Failed to save Java settings');
    }
  };
</script>

<div class="mt-2 ml-4 flex flex-1 flex-col">
  <div class="flex items-center">
    <p class="text-xl">Java</p>
  </div>
  <JavaSettings
    settings={settings?.minecraft.jvm_settings}
    maxMem={settings?.system_max_mem ?? 8192}
    updateSettings={saveSettings}
  />
</div>
