<script lang="ts">
  import { setMode } from 'mode-watcher';
  import {
    ModeWatcher,
    Resizable,
    Separator,
    toast,
    Toaster
  } from 'positron-components/components/ui';
  import '../app.css';
  import Header from '$lib/components/Header.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { onMount } from 'svelte';
  import { account_refresh } from '$lib/tauri/account.svelte';
  import { debounce, rem_to_px } from '$lib/util.svelte';
  import { settings_get, settings_set } from '$lib/tauri/settings.svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  let { children } = $props();

  setMode('dark');

  onMount(async () => {
    if (await account_refresh()) {
      toast.error('Failed to refresh Accounts');
    }
  });

  let innerWidth = $state(0);
  let collapsed = $state(false);
  let settings = $derived(settings_get.value);

  // width is in percentage of the viewport width, and it should be constant at 54rem
  let sidebarMaxWidth = $derived(
    (rem_to_px(54 / 4) / (innerWidth !== 0 ? innerWidth : 1)) * 100
  );
  let sidebarMinWidth = $derived(
    (rem_to_px(16 / 4) / (innerWidth !== 0 ? innerWidth : 1)) * 100
  );

  const debounceSave = debounce((size: number) => {
    if (!settings) return;
    settings.sidebar_width = size;
    settings_set(settings);
  }, 1000);

  const urlDebounce = debounce(() => {
    if (!settings) return;
    settings.url = page.url;
    settings_set(settings);
  }, 1000);

  $effect(() => {
    page.url;
    urlDebounce();
  });

  let init = false;
  $effect(() => {
    if (!init && settings) {
      console.log(settings.url);
      init = true;
      goto(settings.url || '/');
    }
  });
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />
<svelte:window bind:innerWidth />

<div class="flex h-full flex-col">
  <Header />
  <Separator />
  <Resizable.PaneGroup class="flex min-h-0 flex-1" direction="horizontal">
    <Resizable.Pane
      maxSize={sidebarMaxWidth}
      minSize={sidebarMinWidth}
      defaultSize={settings?.sidebar_width || sidebarMaxWidth}
      onResize={(size) => {
        collapsed = Math.abs(size - sidebarMinWidth) < 0.001;
        debounceSave(size);
      }}
    >
      <Sidebar {collapsed} />
    </Resizable.Pane>
    <Resizable.Handle />
    <Resizable.Pane>
      <main class="m-2 min-h-0 flex-1">
        {@render children()}
      </main>
    </Resizable.Pane>
  </Resizable.PaneGroup>
</div>
