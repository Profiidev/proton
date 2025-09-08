<script lang="ts">
  import { Button } from 'positron-components/components/ui';
  import AccountSelector from '../account/AccountSelector.svelte';
  import { Gamepad2, Home, LibraryBig, Settings } from '@lucide/svelte';
  import { page } from '$app/state';
  import { cn } from 'positron-components/utils';
  import { crossfade } from 'svelte/transition';
  import { cubicInOut } from 'svelte/easing';

  const btns = [
    {
      title: 'Home',
      url: '/home',
      icon: Home
    },
    {
      title: 'Profiles',
      url: '/profiles',
      icon: LibraryBig
    },
    {
      title: 'Instances',
      url: '/instances',
      icon: Gamepad2
    },
    {
      title: 'Settings',
      url: '/settings',
      icon: Settings
    }
  ];

  const [send, receive] = crossfade({
    duration: 250,
    easing: cubicInOut
  });
</script>

<div class="flex h-full flex-col p-2 w-54 gap-1">
  {#each btns as btn}
    {@const isActive = page.url.pathname.startsWith(btn.url)}
    <Button
      href={btn.url}
      variant="ghost"
      class={cn(
        'relative h-12 w-full justify-start p-2!',
        !isActive && 'hover:underline'
      )}
    >
      {#if isActive}
        <div
          class="bg-muted absolute inset-0 rounded-md"
          in:send={{ key: 'active-sidebar-tab' }}
          out:receive={{ key: 'active-sidebar-tab' }}
        ></div>
      {/if}
      <div class="relative flex w-full items-center gap-2">
        <btn.icon class="size-full max-w-8" />
        <p class="truncate">{btn.title}</p>
      </div>
    </Button>
  {/each}
  <AccountSelector />
</div>
