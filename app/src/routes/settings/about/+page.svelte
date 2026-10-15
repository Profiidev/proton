<script lang="ts">
  import { is_offline } from '$lib/tauri/offline.svelte';
  import {
    checkForUpdates,
    getUpdateVersion,
    update
  } from '$lib/tauri/updater.svelte';
  import {
    settings_get,
    settings_set,
    type Settings
  } from '$lib/tauri/settings.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { toast } from '@profidev/pleiades/components/util/general';
  import SwitchTile from '$lib/components/settings/SwitchTile.svelte';

  let offline = $derived(is_offline.value);
  let settings = $derived(settings_get.value);
  let version = $state<string>();
  getVersion().then((v) => {
    version = v;
  });

  let version_update = $derived(getUpdateVersion());
  let loading = $state(false);
  let checked = $state(false);

  const check = async () => {
    if (offline) {
      toast.warning(
        'You are currently offline, please reconnect to the internet to check for updates'
      );
      return;
    }

    loading = true;
    let update = await checkForUpdates();
    loading = false;
    checked = true;

    if (update === undefined) {
      toast.message('No updates available');
    }
  };

  const save = async (change: Partial<Settings>) => {
    if (!settings || (await settings_set({ ...settings, ...change }))) {
      toast.error('Failed to save settings');
    }
  };
</script>

<div class="flex max-w-[720px] min-w-0 flex-1 flex-col gap-4">
  <h1 class="text-xl font-semibold tracking-tight">About</h1>
  <div
    class="bg-card ring-foreground/10 flex flex-wrap items-center gap-4 rounded-xl p-4 ring-1"
  >
    <div class="flex flex-[1_1_200px] flex-col">
      <span class="font-medium">Proton</span>
      <span class="text-muted-foreground text-xs">
        Version {version}
        {#if version_update}
          · {version_update} available
        {:else if checked}
          · up to date
        {/if}
      </span>
    </div>
    {#if version_update}
      <Button onclick={update}>Install {version_update}</Button>
    {:else}
      <Button variant="outline" onclick={check} disabled={loading}>
        {#if loading}
          <LoaderCircle class="animate-spin" data-icon="inline-start" />
        {/if}
        Check for updates
      </Button>
    {/if}
  </div>
  {#if settings}
    <SwitchTile
      title="Update automatically"
      description="Install new versions when the launcher starts."
      checked={settings.auto_update ?? false}
      onCheckedChange={(auto_update) => save({ auto_update })}
    />
    <SwitchTile
      title="Hide window controls"
      description="Hide minimize, maximize and close in the title bar, e.g. for tiling window managers."
      checked={settings.hide_window_controls ?? false}
      onCheckedChange={(hide_window_controls) => save({ hide_window_controls })}
    />
  {/if}
</div>
