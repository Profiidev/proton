<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { multiplayer_list } from '../store.svelte';

  let list = $derived(multiplayer_list());
</script>

<QuickPlayList
  value="multiplayer"
  empty_title="No servers found"
  empty_description="Join a server to add it here."
  {list}
/>
