<script lang="ts">
  import { Button } from 'positron-components/components/ui';
  import AccountSelector from './account/AccountSelector.svelte';
  import { Home } from 'lucide-svelte';
  import { goto } from '$app/navigation';
</script>

<div class="flex h-full flex-col p-2">
  <Button
    variant="ghost"
    class="h-12 w-full justify-start p-2"
    onclick={() => goto('/')}
  >
    <Home class="size-8!" />
    <p>Home</p>
  </Button>
  <AccountSelector />
</div>
