<script lang="ts">
  import {
    account_list,
    account_login,
    type ProfileInfo,
  } from "$lib/tauri/account.svelte";
  import { Button } from "positron-components";
  import { onMount } from "svelte";

  let accounts: { [key: string]: ProfileInfo | null } | undefined = $state();
  let result = $state("None");

  const list = async () => {
    accounts = await account_list();
  };

  const add = async () => {
    if (await account_login()) {
      result = "Success";
    } else {
      result = "Error";
    }
    await list();
  };

  onMount(list);
</script>

<main class="h-full">
  {#if accounts && Object.keys(accounts).length !== 0}
    {#each Object.entries(accounts) as [id, profile]}
      <div class="flex">
        <p>{id}</p>
        <p class="ml-2">{profile?.name ?? "Invalid"}</p>
      </div>
    {/each}
  {:else}
    <p>No Accounts</p>
  {/if}
  <Button onclick={add}>Add Account</Button>
  <p>Result: {result}</p>
</main>
