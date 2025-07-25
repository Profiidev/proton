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
  import Header from '$lib/components/nav/Header.svelte';
  import Sidebar from '$lib/components/nav/Sidebar.svelte';
  import { onMount } from 'svelte';
  import { account_refresh } from '$lib/tauri/account.svelte';
  import { debounce, rem_to_px } from '$lib/util.svelte';
  import { settings_get, settings_set } from '$lib/tauri/settings.svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { checkForUpdates } from '$lib/tauri/updater.svelte';
  let { children } = $props();

  setMode('dark');

  onMount(async () => {
    if (await account_refresh()) {
      toast.error('Failed to refresh Accounts');
    }
    let update = await checkForUpdates();
    if (update) {
      toast.message(`Update available: ${update}`, {
        action: {
          label: 'Update',
          onClick: () => {
            goto('/settings/about');
          }
        }
      });
    }
  });

  let innerWidth = $state(0);
  let collapsed = $state(false);
  let settings = $derived(settings_get.value);
  let pane = $state<Resizable.Pane>();

  // width is in percentage of the viewport width, and it should be constant at 54rem
  let sidebarMaxWidth = $derived(
    (rem_to_px(54 / 4) / (innerWidth !== 0 ? innerWidth : 1)) * 100
  );
  let sidebarMinWidth = $derived(
    (rem_to_px(16 / 4) / (innerWidth !== 0 ? innerWidth : 1)) * 100
  );

  let previousWidth = 0;
  $effect(() => {
    innerWidth;

    setTimeout(() => {
      if (previousWidth !== 0) {
        pane?.resize(
          (previousWidth * (settings?.sidebar_width || 0)) / innerWidth
        );
      }

      previousWidth = innerWidth;
    });
  });

  const debounceSave = debounce((size: number) => {
    if (!settings) return;
    settings_set({
      ...settings,
      sidebar_width: size
    });
  }, 500);

  const urlDebounce = debounce(() => {
    if (!settings) return;
    settings.url = page.url;
    settings_set(settings);
  }, 500);

  $effect(() => {
    page.url;
    urlDebounce();
  });

  let init = false;
  $effect(() => {
    if (!init && settings) {
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
      bind:this={pane}
    >
      <Sidebar {collapsed} />
    </Resizable.Pane>
    <Resizable.Handle />
    <Resizable.Pane>
      <main class="h-full min-h-0 p-2">
        {@render children()}
      </main>
    </Resizable.Pane>
  </Resizable.PaneGroup>
</div>
