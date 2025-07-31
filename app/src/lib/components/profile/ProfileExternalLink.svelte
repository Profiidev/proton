<script lang="ts">
  import { goto } from '$app/navigation';
  import { ExternalLink } from '@lucide/svelte';
  import { Button } from 'positron-components/components/ui';
</script>

<div class="mt-2 flex justify-center">
  <Button
    variant="outline"
    onclick={() => goto('/profiles')}
    class="text-md inline-flex w-fit cursor-pointer p-0"
  >
    Profiles
    <ExternalLink />
  </Button>
</div>
