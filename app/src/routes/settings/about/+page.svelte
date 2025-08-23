<script lang="ts">
  import { is_offline } from '$lib/tauri/offline.svelte';
  import {
    checkForUpdates,
    getUpdateVersion,
    update
  } from '$lib/tauri/updater.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { Button, toast } from 'positron-components/components/ui';

  let offline = $derived(is_offline.value);
  let version = $state<string>();
  getVersion().then((v) => {
    version = v;
  });

  let version_update = $derived(getUpdateVersion());
  let loading = $state(false);

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

    if (update === undefined) {
      toast.message('No updates available');
    }
  };
</script>

<div class="mt-2 ml-4 flex-1">
  <p class="text-xl">Proton</p>
  <div class="flex items-center gap-2">
    <p class="text-md">Version: {version}</p>
    {#if !version_update}
      <Button class="ml-auto cursor-pointer" onclick={check} disabled={loading}
        >Check for Updates
        {#if loading}
          <LoaderCircle />
        {/if}
      </Button>
    {:else}
      <Button class="ml-auto cursor-pointer" onclick={update}
        >Install Update: {version_update}</Button
      >
    {/if}
  </div>
</div>
