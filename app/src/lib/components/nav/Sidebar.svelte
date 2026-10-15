<script lang="ts">
  import AccountSelector from '../account/AccountSelector.svelte';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Gamepad2, Home, LibraryBig, Settings } from '@lucide/svelte';
  import { page } from '$app/state';
  import { cn } from '@profidev/pleiades/utils';
  import { crossfade } from 'svelte/transition';
  import { cubicInOut } from 'svelte/easing';
  import { instance_list } from '$lib/tauri/instance.svelte';

  let running = $derived(instance_list.value?.length ?? 0);

  const top = [
    { title: 'Home', url: '/home', icon: Home },
    { title: 'Profiles', url: '/profiles', icon: LibraryBig },
    { title: 'Instances', url: '/instances', icon: Gamepad2 }
  ];
  const settings = { title: 'Settings', url: '/settings', icon: Settings };

  const [send, receive] = crossfade({
    duration: 250,
    easing: cubicInOut
  });
</script>

<nav
  aria-label="Main"
  class="flex h-full w-16 shrink-0 flex-col gap-0.5 p-2 md:w-54"
>
  {#each top as btn (btn.url)}
    {@render link(btn)}
  {/each}
  <div class="min-h-4 flex-1"></div>
  {@render link(settings)}
  <Separator class="my-1" />
  <AccountSelector />
</nav>

{#snippet link(btn: (typeof top)[number])}
  {@const isActive = page.url.pathname.startsWith(btn.url)}
  <a
    href={btn.url}
    aria-current={isActive ? 'page' : undefined}
    class={cn(
      'text-muted-foreground relative flex h-10 shrink-0 items-center gap-3 rounded-md px-2.5 text-sm font-medium whitespace-nowrap transition-colors',
      isActive ? 'text-foreground' : 'hover:bg-accent/60 hover:text-foreground'
    )}
  >
    {#if isActive}
      <div
        class="bg-muted absolute inset-0 rounded-md"
        in:send={{ key: 'active-sidebar-tab' }}
        out:receive={{ key: 'active-sidebar-tab' }}
      ></div>
    {/if}
    <btn.icon class="relative size-5 shrink-0" />
    <span class="relative hidden truncate md:block">{btn.title}</span>
    {#if btn.url === '/instances' && running > 0}
      <span
        class="relative ml-auto hidden h-5 items-center gap-1 rounded-full bg-green-500/15 px-2 text-xs font-medium text-green-500 md:inline-flex"
      >
        <span class="size-1.5 rounded-full bg-green-500"></span>{running}
      </span>
    {/if}
  </a>
{/snippet}
