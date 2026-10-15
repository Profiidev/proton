<script lang="ts">
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import * as Breadcrumb from '@profidev/pleiades/components/ui/breadcrumb';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { Minus, Square, X } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { page } from '$app/state';
  import { profile_list } from '$lib/tauri/profile.svelte';
  import { is_offline, try_reconnect } from '$lib/tauri/offline.svelte';
  import { instance_list } from '$lib/tauri/instance.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { breadcrumbs } from './breadcrumbs';
  import DownloadsPanel from './DownloadsPanel.svelte';

  let offline = $derived(is_offline.value);
  let hide_controls = $derived(settings_get.value?.hide_window_controls);
  let items = $derived(
    breadcrumbs(
      page.url.pathname,
      page.url.search,
      profile_list.value,
      instance_list.value
    )
  );

  const reconnect = async () => {
    if (await try_reconnect()) {
      toast.success('Reconnected successfully');
    } else {
      toast.error('Failed to reconnect');
    }
  };

  const close = () => {
    getCurrentWindow().close();
  };

  const minimize = () => {
    getCurrentWindow().minimize();
  };

  const maximize = () => {
    getCurrentWindow().toggleMaximize();
  };
</script>

<header data-tauri-drag-region class="flex h-10 shrink-0 items-center text-sm">
  <img src="/icon.svg" alt="Proton Icon" class="mx-2 size-7 select-none" />
  <p class="mr-4 font-medium">Proton</p>
  <Separator orientation="vertical" class="h-5! self-center!" />
  <Breadcrumb.Root class="mx-4 min-w-0 overflow-hidden">
    <Breadcrumb.List class="flex-nowrap">
      {#each items as item, i (item.href)}
        {#if i > 0}
          <Breadcrumb.Separator />
        {/if}
        <Breadcrumb.Item>
          {#if i === items.length - 1}
            <Breadcrumb.Page>{item.label}</Breadcrumb.Page>
          {:else}
            <Breadcrumb.Link href={item.href}>{item.label}</Breadcrumb.Link>
          {/if}
        </Breadcrumb.Item>
      {/each}
    </Breadcrumb.List>
  </Breadcrumb.Root>
  <div class="ml-auto flex items-center gap-1 pr-1">
    {#if offline}
      <Badge
        class="mr-2 cursor-pointer rounded-full"
        variant="destructive"
        onclick={reconnect}>Offline</Badge
      >
    {/if}
    <DownloadsPanel />
    {#if !hide_controls}
      <Button
        class="size-8 rounded-full"
        size="icon"
        variant="ghost"
        aria-label="Minimize"
        onclick={minimize}
      >
        <Minus class="size-4.5!" />
      </Button>
      <Button
        class="size-8 rounded-full"
        size="icon"
        variant="ghost"
        aria-label="Maximize"
        onclick={maximize}
      >
        <Square />
      </Button>
      <Button
        class="hover:bg-destructive! size-8 rounded-full"
        size="icon"
        variant="ghost"
        aria-label="Close"
        onclick={close}
      >
        <X class="size-4.5!" />
      </Button>
    {/if}
  </div>
</header>
