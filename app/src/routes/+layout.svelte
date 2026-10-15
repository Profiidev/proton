<script lang="ts">
  import { setMode } from 'mode-watcher';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Toaster } from '@profidev/pleiades/components/ui/sonner';
  import {
    ModeWatcher,
    toast
  } from '@profidev/pleiades/components/util/general';
  import '../app.css';
  import Header from '$lib/components/nav/Header.svelte';
  import Sidebar from '$lib/components/nav/Sidebar.svelte';
  import { onMount } from 'svelte';
  import { account_refresh } from '$lib/tauri/account.svelte';
  import { debounce } from '$lib/util.svelte';
  import {
    settings_get,
    settings_set,
    type Settings
  } from '$lib/tauri/settings.svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { checkForUpdates, update } from '$lib/tauri/updater.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen_instance_crash } from '$lib/tauri/instance.svelte';
  import {
    is_offline,
    listen_manifest_refresh_error
  } from '$lib/tauri/offline.svelte';
  import { webviewWindow } from '@tauri-apps/api';

  let { children } = $props();

  setMode('dark');

  let instance_crash_unsub = () => {};
  let manifest_refresh_error_unsub = () => {};
  let offline = $derived(is_offline.value);

  onMount(async () => {
    webviewWindow.getCurrentWebviewWindow().show();
    instance_crash_unsub = await listen_instance_crash();
    manifest_refresh_error_unsub = await listen_manifest_refresh_error();

    if (offline) {
      toast.warning('You are currently offline, some features may not work', {
        duration: 10000
      });
    }

    if (await account_refresh()) {
      toast.error('Failed to refresh Accounts');
    }
    let version = await checkForUpdates();
    if (version) {
      const current = await invoke<Settings>('settings_get').catch(() => {});
      if (current?.auto_update) {
        await update();
        return;
      }
      toast.message(`Update available: ${version}`, {
        action: {
          label: 'Update',
          onClick: () => {
            goto('/settings/about');
          }
        }
      });
    }
  });
  onMount(() => {
    return () => {
      instance_crash_unsub();
      manifest_refresh_error_unsub();
    };
  });

  let settings = $derived(settings_get.value);

  const urlDebounce = debounce(() => {
    if (!settings) return;
    settings.url = page.url;
    settings_set(settings);
  }, 500);

  $effect(() => {
    page.url;
    urlDebounce();
  });
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="flex h-full flex-col">
  <Header />
  <Separator />
  <div class="flex min-h-0 w-full grow">
    <Sidebar />
    <Separator orientation="vertical" />
    <main class="h-full min-h-0 min-w-0 grow overflow-auto p-2">
      {@render children()}
    </main>
  </div>
</div>
