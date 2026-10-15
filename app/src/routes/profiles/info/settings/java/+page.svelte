<script lang="ts">
  import JavaSettings from '$lib/components/settings/JavaSettings.svelte';
  import SwitchTile from '$lib/components/settings/SwitchTile.svelte';
  import { memory_of_total } from '$lib/components/settings/settings';
  import {
    profile_update,
    ProfileError,
    type JvmSettings
  } from '$lib/tauri/profile.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { getProfile } from '../../store.svelte';
  import { toast } from 'svelte-sonner';

  let profile = $derived(getProfile());
  let settings = $derived(settings_get.value);
  let maxMem = $derived(settings?.system_max_mem ?? 8192);

  const saveSettings = async (jvm: JvmSettings, use_local_jvm: boolean) => {
    if (!profile) return;

    let res = await profile_update({ ...profile, use_local_jvm, jvm });
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile');
    }
  };
</script>

{#if profile && settings}
  <SwitchTile
    title="Use profile Java settings"
    description="Override the global Java settings for this profile only."
    checked={profile.use_local_jvm}
    onCheckedChange={(value) =>
      saveSettings(profile.jvm ?? settings.minecraft.jvm_settings, value)}
  />
  <JavaSettings
    settings={profile.jvm ?? settings.minecraft.jvm_settings}
    {maxMem}
    memoryLabel={(mb) => memory_of_total(mb, maxMem)}
    memoryHint="Most modded profiles run well with 4 to 6 GB."
    updateSettings={(jvm) => saveSettings(jvm, profile.use_local_jvm)}
    disabled={!profile.jvm || !profile.use_local_jvm}
  />
{/if}
