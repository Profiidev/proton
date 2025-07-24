<script lang="ts">
  import {
    Breadcrumb,
    Button,
    Separator
  } from 'positron-components/components/ui';
  import { Minus, Square, X } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { page } from '$app/state';
  import { profile_list } from '$lib/tauri/profile.svelte';

  let items: { label: string; href: string }[] = $state([]);
  let profiles = $derived(profile_list.value);

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
  <p class="mx-4">Proton</p>
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
  <Button
    class="ml-auto size-8 cursor-pointer rounded-full"
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
