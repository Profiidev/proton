<script lang="ts">
  import { Button } from 'positron-components/components/ui';
  import AccountSelector from './account/AccountSelector.svelte';
  import { Home, LibraryBig, Settings } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';

  interface Props {
    collapsed: boolean;
  }

  let { collapsed }: Props = $props();

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
      title: 'Settings',
      url: '/settings',
      icon: Settings
    }
  ];
</script>

<div class="flex h-full flex-col p-2">
  {#each btns as btn}
    <Button
      variant="ghost"
      class={'h-12 w-full justify-start p-2!' +
        (page.url.pathname.startsWith(btn.url)
          ? ' bg-accent text-accent-foreground'
          : '')}
      onclick={() => goto(btn.url)}
    >
      <btn.icon class="size-full max-w-8" />
      {#if !collapsed}
        <p class="truncate">{btn.title}</p>
      {/if}
    </Button>
  {/each}
  <AccountSelector {collapsed} />
</div>
