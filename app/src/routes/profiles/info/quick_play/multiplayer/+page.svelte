<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { multiplayer_list } from '../store.svelte';

  let list = $derived(multiplayer_list());
</script>

<QuickPlayList value="multiplayer" name="multiplayer" join="server" {list} />
