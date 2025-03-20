<script lang="ts">
  import { setMode } from 'mode-watcher';
  import {
    ModeWatcher,
    Separator,
    toast,
    Toaster
  } from 'positron-components/components/ui';
  import '../app.css';
  import Header from '$lib/components/Header.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { onMount } from 'svelte';
  import { account_refresh } from '$lib/tauri/account.svelte';
  let { children } = $props();

  setMode('dark');

  onMount(async () => {
    if (!(await account_refresh())) {
      toast.error('Failed to refresh Accounts');
    }
  });
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="flex h-full flex-col">
  <Header />
  <Separator />
  <div class="flex min-h-0 flex-1">
    <Sidebar />
    <Separator orientation="vertical" />
    <main class="m-2 min-h-0 flex-1">
      {@render children()}
    </main>
  </div>
</div>
