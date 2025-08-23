<script lang="ts">
  import {
    profile_update,
    ProfileError,
    type GameSettings as GameSettingsType
  } from '$lib/tauri/profile.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { getProfile } from '../../store.svelte';
  import SwitchTooltip from '$lib/components/form/SwitchTooltip.svelte';
  import { toast } from 'svelte-sonner';
  import { Separator } from 'positron-components/components/ui';
  import GameSettings from '$lib/components/settings/GameSettings.svelte';

  let profile = $derived(getProfile());
  let settings = $derived(settings_get.value);

  const saveSettings = async (
    new_settings: GameSettingsType,
    use_local_game: boolean
  ) => {
    if (!profile) return;

    let res = await profile_update({
      ...profile,
      use_local_game,
      game: new_settings
    });
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile');
    }
  };
</script>

{#if profile && settings}
  <SwitchTooltip
    class="mt-2"
    id="local-game"
    bind:checked={profile.use_local_game}
    label="Local Game Settings"
    tooltip="Overwrite global Game settings with local ones"
    onCheckedChange={(value) => {
      saveSettings(profile.game ?? settings.minecraft.game_settings, value);
    }}
  />
  <Separator class="my-2" />
  <GameSettings
    settings={profile.game ?? settings.minecraft.game_settings}
    updateSettings={(settings) =>
      saveSettings(settings, profile.use_local_game)}
    disabled={!profile.game || !profile.use_local_game}
  />
{/if}
