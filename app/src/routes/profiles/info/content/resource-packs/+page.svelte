<script lang="ts">
  import ContentList from '$lib/components/content/ContentList.svelte';
  import { getProfile } from '../../store.svelte';

  let profile = $derived(getProfile());
</script>

{#if profile}
  <ContentList kind="resourcepack" {profile} />
{/if}
