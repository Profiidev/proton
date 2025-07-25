<script lang="ts">
  import {
    checkForUpdates,
    getUpdateVersion,
    update
  } from '$lib/tauri/updater.svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { Button } from 'positron-components';

  let version = $state<string>();
  getVersion().then((v) => {
    version = v;
  });
  checkForUpdates();

  let version_update = $derived(getUpdateVersion());
</script>

<div class="mt-2 ml-4 flex-1">
  <p class="text-xl">Proton</p>
  <div class="flex items-center gap-2">
    <p class="text-md">Version: {version}</p>
    {#if !version_update}
      <Button class="ml-auto cursor-pointer" onclick={checkForUpdates}
        >Check for Updates</Button
      >
    {:else}
      <Button class="ml-auto cursor-pointer" onclick={update}
        >Install Update: {version_update}</Button
      >
    {/if}
  </div>
</div>
