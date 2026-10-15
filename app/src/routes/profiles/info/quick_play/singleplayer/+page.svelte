<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { singleplayer_list } from '../store.svelte';

  let list = $derived(singleplayer_list());
</script>

<QuickPlayList
  value="singleplayer"
  empty_title="No worlds found"
  empty_description="Join a singleplayer world to add it here."
  {list}
/>
