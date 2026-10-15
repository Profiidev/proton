<script lang="ts">
  import QuickPlayList from '../QuickPlayList.svelte';
  import { realms_list } from '../store.svelte';

  let list = $derived(realms_list());
</script>

<QuickPlayList
  value="realms"
  empty_title="No realms found"
  empty_description="This account does not own or belong to a Realm."
  {list}
/>
