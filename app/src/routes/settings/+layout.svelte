<script lang="ts">
  import { page } from '$app/state';
  import SimpleSidebar from '$lib/components/nav/SimpleSidebar.svelte';
  import { cn } from '@profidev/pleiades/utils';

  let { children } = $props();

  const items = [
    { href: '/settings/accounts', title: 'Accounts' },
    { href: '/settings/skins', title: 'Skins' },
    { href: '/settings/capes', title: 'Capes' },
    { href: '/settings/minecraft', title: 'Minecraft' },
    { href: '/settings/java', title: 'Java' },
    { href: '/settings/about', title: 'About' }
  ];
</script>

<div class="flex flex-wrap items-start gap-5 px-5 pt-4 pb-6">
  <SimpleSidebar {items} class="w-40 shrink-0" />
  <div
    class={cn(
      'flex min-w-0 flex-[999_1_480px] flex-col gap-3',
      !page.url.pathname.startsWith('/settings/skins') && 'max-w-180'
    )}
  >
    {@render children()}
  </div>
</div>
