<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { singleplayer_list } from '../store.svelte';

  let list = $derived(singleplayer_list());
</script>

<QuickPlayList
  value="singleplayer"
  name="singleplayer"
  join="singleplayer world"
  {list}
/>
