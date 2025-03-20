<script lang="ts">
    import { account_active } from '$lib/tauri/account.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Button } from 'positron-components/components/ui';
  
  let active = $derived(account_active.value);

  const download = async () => {
    let res = await invoke('versions_launch', {
      version: '1.0',
      account: active,
    });
    console.log(res);
  };
</script>

<p>Home</p>
<Button onclick={download}>Launch</Button>
