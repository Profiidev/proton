<script lang="ts">
  import { Button } from 'positron-components/components/ui';
  import AccountSelector from './account/AccountSelector.svelte';
  import { Home, LibraryBig, Settings } from 'lucide-svelte';
  import { goto } from '$app/navigation';

  const btns = [
    {
      title: 'Home',
      url: '/',
      icon: Home
    },
    {
      title: 'Profiles',
      url: '/profiles',
      icon: LibraryBig
    },
    {
      title: 'Settings',
      url: '/settings/accounts',
      icon: Settings
    }
  ];
</script>

<div class="flex h-full flex-col p-2">
  {#each btns as btn}
    <Button
      variant="ghost"
      class="h-12 w-full justify-start p-2"
      onclick={() => goto(btn.url)}
    >
      <btn.icon class="size-8!" />
      <p>{btn.title}</p>
    </Button>
  {/each}
  <AccountSelector />
</div>
