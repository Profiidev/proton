<script lang="ts">
  import { goto } from "$app/navigation";
  import {
    account_active,
    account_list,
    account_set_active,
    State,
    type Accounts,
    type ProfileInfo,
  } from "$lib/tauri/account.svelte";
  import { CircleHelp, Settings } from "lucide-svelte";
  import { Popover, Button } from "positron-components/components/ui";
  import AccountImage from "./AccountImage.svelte";

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active: string | undefined = $derived(account_active.value);
  let open = $state(false);

  const edit = (e: Event) => {
    e.stopPropagation();
    goto("/settings/accounts");
  };
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class="ml-auto h-12 w-50 rounded-lg flex items-center mt-auto"
  >
    {#if active && accounts && accounts[active]}
      {@render account({
        info: accounts[active],
        onclick: () => {},
        edit: true,
      })}
    {:else}
      <Button class="justify-start h-12 p-2 w-full" variant="ghost">
        <div class="size-10 min-w-10 flex items-center justify-center">
          <CircleHelp />
        </div>
        <p class="truncate text-start text-sm">No account active</p>
        {@render edit_btn()}
      </Button>
    {/if}
  </Popover.Trigger>
  <Popover.Content class="w-50 flex flex-col p-1">
    {#if accounts}
      {#each Object.entries(accounts).sort( (a, b) => a[0].localeCompare(b[0]), ) as [id, info]}
        {#if info}
          {@render account({
            info,
            onclick: async () => {
              await account_set_active(id);
              account_active.update();
              open = false;
            },
          })}
        {:else}
          <div class="flex items-center h-14 p-2 w-full">
            <div class="size-10 min-w-10 flex items-center justify-center">
              <CircleHelp />
            </div>
            <div class="flex flex-col justify-start flex-grow-1 min-w-0">
              <p class="truncate text-start text-sm">Login failed</p>
              <p class="truncate text-start text-sm text-muted-foreground">
                {id}
              </p>
            </div>
          </div>
        {/if}
      {/each}
    {:else}
      <p>No Accounts</p>
    {/if}
  </Popover.Content>
</Popover.Root>

{#snippet account({
  info,
  onclick,
  edit,
}: {
  info: ProfileInfo;
  onclick: () => void;
  edit?: boolean;
})}
  {@const skin_url = info.skins.find((s) => s.state === State.Active)?.url}
  <Button class="justify-start p-2 h-12 w-full" variant="ghost" {onclick}>
    <AccountImage {skin_url} />
    <div class="flex flex-col justify-start flex-grow-1 min-w-0">
      <p class="truncate text-start text-sm">{info.name}</p>
      <p class="truncate text-start text-sm text-muted-foreground">{info.id}</p>
    </div>
    {#if edit}
      {@render edit_btn()}
    {/if}
  </Button>
{/snippet}

{#snippet edit_btn()}
  <Button
    size="icon"
    variant="ghost"
    class="size-8 min-w-8 hover:bg-muted-foreground"
    onclick={edit}
  >
    <Settings class="size-5!" />
  </Button>
{/snippet}
