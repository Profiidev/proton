<script lang="ts">
  import SimpleSidebar from 'positron-components/components/nav/simple-sidebar.svelte';

  let { children } = $props();

  let items = [
    {
      href: '/settings/accounts',
      title: 'Accounts'
    },
    {
      href: '/settings/skins',
      title: 'Skins'
    },
    {
      href: '/settings/capes',
      title: 'Capes'
    },
    {
      href: '/settings/minecraft',
      title: 'Minecraft'
    },
    {
      href: '/settings/java',
      title: 'Java'
    },
    {
      href: '/settings/about',
      title: 'About'
    }
  ];
</script>

<div class="flex h-full flex-col lg:flex-row">
  <aside class="lg:w-52 lg:max-w-32">
    <SimpleSidebar {items} class="" />
  </aside>
  <div
    class="flex min-h-0 flex-1 space-y-8 lg:h-full lg:space-y-0 lg:space-x-12"
  >
    {@render children()}
  </div>
</div>
