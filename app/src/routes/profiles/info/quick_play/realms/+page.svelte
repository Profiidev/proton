<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { realms_list } from '../store.svelte';

  let list = $derived(realms_list());
</script>

<QuickPlayList value="realms" name="realms" join="realm" {list} />
