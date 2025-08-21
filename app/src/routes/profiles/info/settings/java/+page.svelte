<script lang="ts">
  import JavaSettings from '$lib/components/settings/JavaSettings.svelte';
  import {
    profile_update,
    ProfileError,
    type JvmSettings
  } from '$lib/tauri/profile.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { getProfile } from '../../store.svelte';
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';
  import { toast } from 'svelte-sonner';
  import { Separator } from 'positron-components/components/ui';

  let profile = $derived(getProfile());
  let settings = $derived(settings_get.value);

  const saveSettings = async (
    new_settings: JvmSettings,
    use_local_jvm: boolean
  ) => {
    if (!profile) return;

    let res = await profile_update({
      ...profile,
      use_local_jvm,
      jvm: new_settings
    });
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile');
    }
  };
</script>

{#if profile && settings}
  <SwitchTooltip
    class="mt-2"
    id="local-jvm"
    bind:checked={profile.use_local_jvm}
    label="Local Java Settings"
    tooltip="Overwrite global Java settings with local ones"
    onCheckedChange={(value) => {
      saveSettings(profile.jvm ?? settings.minecraft.jvm_settings, value);
    }}
  />
  <Separator class="my-2" />
  <JavaSettings
    settings={profile.jvm ?? settings.minecraft.jvm_settings}
    updateSettings={(settings) => saveSettings(settings, profile.use_local_jvm)}
    maxMem={settings.system_max_mem ?? 8192}
    disabled={!profile.jvm || !profile.use_local_jvm}
  />
{/if}
