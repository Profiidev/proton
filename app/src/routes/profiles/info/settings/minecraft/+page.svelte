<script lang="ts">
  import {
    profile_update,
    ProfileError,
    type GameSettings as GameSettingsType
  } from '$lib/tauri/profile.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { getProfile } from '../../store.svelte';
  import { toast } from 'svelte-sonner';
  import GameSettings from '$lib/components/settings/GameSettings.svelte';
  import SwitchTile from '$lib/components/settings/SwitchTile.svelte';

  let profile = $derived(getProfile());
  let settings = $derived(settings_get.value);

  const saveSettings = async (
    game: GameSettingsType,
    use_local_game: boolean
  ) => {
    if (!profile) return;

    let res = await profile_update({ ...profile, use_local_game, game });
    if (res === ProfileError.Other) {
      toast.error('Failed to update profile');
    }
  };
</script>

{#if profile && settings}
  <SwitchTile
    title="Use profile game settings"
    description="Override the global Minecraft settings for this profile only."
    checked={profile.use_local_game}
    onCheckedChange={(value) =>
      saveSettings(profile.game ?? settings.minecraft.game_settings, value)}
  />
  <GameSettings
    settings={profile.game ?? settings.minecraft.game_settings}
    updateSettings={(game) => saveSettings(game, profile.use_local_game)}
    disabled={!profile.game || !profile.use_local_game}
  />
{/if}
