<script lang="ts">
  import { Badge } from 'positron-components/components/ui/badge';
  import * as Breadcrumb from 'positron-components/components/ui/breadcrumb';
  import { Button } from 'positron-components/components/ui/button';
  import { Separator } from 'positron-components/components/ui/separator';
  import { toast } from 'positron-components/components/util/general';
  import { Minus, Square, X } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { page } from '$app/state';
  import { profile_list } from '$lib/tauri/profile.svelte';
  import { is_offline, try_reconnect } from '$lib/tauri/offline.svelte';

  let items: { label: string; href: string }[] = $state([]);
  let profiles = $derived(profile_list.value);
  let offline = $derived(is_offline.value);

  const calcItems = async () => {
    let parts = page.url.pathname.split('/').slice(1);

    let items = [];
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      let label = part
        .split('_')
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');

      if (
        label === 'Info' &&
        items.length > 0 &&
        items[items.length - 1].label === 'Profiles'
      ) {
        let id = page.url.searchParams.get('id');
        if (id) {
          const profile = profiles?.find((p) => p.id === id);
          if (profile) {
            label = profile.name;
          }
        }
      }

      let href = '/' + parts.slice(0, i + 1).join('/') + page.url.search;
      items.push({ label, href });
    }
    return items;
  };

  $effect(() => {
    page.url.pathname;
    page.url.search;
    profiles;

    calcItems().then((newItems) => {
      items = newItems;
    });
  });

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

<header data-tauri-drag-region class="flex h-10 items-center">
  <img src="/icon.svg" alt="Proton Icon" class="mx-2 h-full p-1 select-none" />
  <p class="mr-4">Proton</p>
  <Separator orientation="vertical" />
  <Breadcrumb.Root class="mx-4 min-w-0 overflow-hidden">
    <Breadcrumb.List class="flex-nowrap">
      {#each items as item, i}
        {#if i > 0}
          <Breadcrumb.Separator />
        {/if}
        <Breadcrumb.Item>
          <Breadcrumb.Link href={item.href}>{item.label}</Breadcrumb.Link>
        </Breadcrumb.Item>
      {/each}
    </Breadcrumb.List>
  </Breadcrumb.Root>
  {#if offline}
    <Badge
      class="ml-auto cursor-pointer rounded-full"
      variant="destructive"
      onclick={reconnect}>Offline</Badge
    >
    <Separator orientation="vertical" class="mr-2 ml-4" />
  {/if}
  <Button
    class={`size-8 cursor-pointer rounded-full ${offline ? '' : 'ml-auto'}`}
    size="icon"
    variant="ghost"
    onclick={minimize}
  >
    <Minus class="size-5!" />
  </Button>
  <Button
    class="size-8 cursor-pointer rounded-full"
    size="icon"
    variant="ghost"
    onclick={maximize}
  >
    <Square />
  </Button>
  <Button
    class="hover:bg-destructive! mr-1 size-8 cursor-pointer rounded-full"
    size="icon"
    variant="ghost"
    onclick={close}
  >
    <X class="size-5!" />
  </Button>
</header>
