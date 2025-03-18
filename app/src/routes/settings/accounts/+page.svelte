<script lang="ts">
  import AccountImage from "$lib/components/account/AccountImage.svelte";
  import {
    account_active,
    account_list,
    account_login,
    account_remove,
    account_set_active,
    State,
    type Accounts,
  } from "$lib/tauri/account.svelte";
  import { LoaderCircle, Plus, Trash } from "lucide-svelte";
  import {
    toast,
    Badge,
    Button,
    Separator,
  } from "positron-components/components/ui";

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active = $derived(account_active.value);
  let add_loading = $state(false);
  $inspect(active).with(console.log);

  const change = async (id: string) => {
    if (await account_set_active(id)) {
      account_active.update();
    } else {
      toast.error("Failed to switch Account");
    }
  };

  const remove = async (id: string) => {
    if (await account_remove(id)) {
      account_list.update();
      toast.success("Successfully removed Account");
    } else {
      toast.error("Failed to remove Account");
    }
  };

  const add = async () => {
    add_loading = true;
    if (await account_login()) {
      account_list.update();
      toast.success("Successfully added Account");
    } else {
      toast.error("Failed to add Account");
    }
    add_loading = false;
  };
</script>

<div class="ml-4 mt-2">
  <div class="flex items-center">
    <p class="text-xl">Accounts</p>
    <Button
      size="icon"
      class="size-8 ml-auto mr-3.5"
      onclick={add}
      disabled={add_loading}
    >
      {#if add_loading}
        <LoaderCircle class="animate-spin" />
      {:else}
        <Plus />
      {/if}
    </Button>
  </div>
  <div class="rounded-lg border mt-2">
    {#if accounts}
      {#each Object.entries(accounts).sort( (a, b) => a[0].localeCompare(b[0]), ) as [id, info], i}
        <Button
          class="gap-2 w-full p-3 h-14"
          variant="ghost"
          onclick={() => change(id)}
        >
          {#if info}
            {@const skin_url = info.skins.find(
              (s) => s.state === State.Active,
            )?.url}
            <AccountImage {skin_url} />
            <div class="flex flex-col justify-start flex-grow-1 min-w-0">
              <p class="truncate text-start text-sm">{info.name}</p>
              <p class="truncate text-start text-sm text-muted-foreground">
                {info.id}
              </p>
            </div>
            {#if id === active}
              <Badge class="mr-2">Selected</Badge>
            {/if}
            <Button
              variant="destructive"
              size="icon"
              class="size-8"
              onclick={(e) => {
                e.stopPropagation();
                remove(id);
              }}
            >
              <Trash />
            </Button>
          {:else}
            <p>{id}</p>
          {/if}
        </Button>
        {#if i !== Object.entries(accounts).length - 1}
          <Separator />
        {/if}
      {/each}
    {:else}
      <p class="p-4">No Accounts available</p>
    {/if}
  </div>
</div>
