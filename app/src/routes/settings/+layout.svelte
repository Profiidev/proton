<script lang="ts">
  import { SimpleSidebar } from "positron-components/components";

  let { children } = $props();

  let items = [
    {
      href: "/settings/accounts",
      title: "Accounts",
    },
    {
      href: "/settings/skins",
      title: "Skins",
    },
    {
      href: "/settings/capes",
      title: "Capes",
    },
  ];
</script>

<div class="flex flex-col lg:flex-row h-full">
  <aside class="lg:max-w-32 lg:w-52">
    <SimpleSidebar {items} class="" />
  </aside>
  <div
    class="flex-1 flex min-h-0 lg:h-full lg:space-x-12 lg:space-y-0 space-y-8"
  >
    {@render children()}
  </div>
</div>
