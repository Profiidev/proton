<script lang="ts">
  import {
    checkForUpdates,
    getUpdateVersion,
    update
  } from '$lib/tauri/updater.svelte';
  import { LoaderCircle } from '@lucide/svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { Button, toast } from 'positron-components';

  let version = $state<string>();
  getVersion().then((v) => {
    version = v;
  });

  let version_update = $derived(getUpdateVersion());
  let loading = $state(false);

  const check = async () => {
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
