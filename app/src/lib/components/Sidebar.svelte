<script lang="ts">
  import { Button } from "positron-components/components/ui";
  import AccountSelector from "./account/AccountSelector.svelte";
  import { Home } from "lucide-svelte";
  import { goto } from "$app/navigation";
</script>

<div class="p-2 h-full flex flex-col">
  <Button
    variant="ghost"
    class="h-12 justify-start p-2 w-full"
    onclick={() => goto("/")}
  >
    <Home class="size-8!" />
    <p>Home</p>
  </Button>
  <AccountSelector />
</div>
