<script lang="ts">
  import { account_list, account_login } from "$lib/tauri/account.svelte";
  import { Button } from "positron-components";

  let result = $state("None");

  const add = async () => {
    if (await account_login()) {
      result = "Success";
    } else {
      result = "Error";
    }
    account_list.update();
  };
</script>

<main class="h-full">
  <Button onclick={add}>Add Account</Button>
  <p>Result: {result}</p>
</main>
