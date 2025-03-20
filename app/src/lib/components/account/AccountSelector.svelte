<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    account_active,
    account_list,
    account_set_active,
    State,
    type Accounts,
    type ProfileInfo
  } from '$lib/tauri/account.svelte';
  import { CircleHelp, Settings } from 'lucide-svelte';
  import { Popover, Button } from 'positron-components/components/ui';
  import AccountImage from './AccountImage.svelte';

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active: string | undefined = $derived(account_active.value);
  let open = $state(false);

  const edit = (e: Event) => {
    e.stopPropagation();
    goto('/settings/accounts');
  };
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class="mt-auto ml-auto flex h-12 w-50 items-center rounded-lg"
  >
    {#if active && accounts && accounts[active]}
      {@render account({
        info: accounts[active],
        onclick: () => {},
        edit: true
      })}
    {:else}
      <Button class="h-12 w-full justify-start p-2" variant="ghost">
        <div class="flex size-10 min-w-10 items-center justify-center">
          <CircleHelp />
        </div>
        <p class="truncate text-start text-sm">No account active</p>
        {@render edit_btn()}
      </Button>
    {/if}
  </Popover.Trigger>
  <Popover.Content class="flex w-50 flex-col p-1">
    {#if accounts}
      {#each Object.entries(accounts).sort( (a, b) => a[0].localeCompare(b[0]) ) as [id, info]}
        {#if info}
          {@render account({
            info,
            onclick: async () => {
              await account_set_active(id);
              open = false;
            }
          })}
        {:else}
          <div class="flex h-14 w-full items-center p-2">
            <div class="flex size-10 min-w-10 items-center justify-center">
              <CircleHelp />
            </div>
            <div class="flex min-w-0 flex-1 flex-col justify-start">
              <p class="truncate text-start text-sm">Login failed</p>
              <p class="text-muted-foreground truncate text-start text-sm">
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
  edit
}: {
  info: ProfileInfo;
  onclick: () => void;
  edit?: boolean;
})}
  {@const skin_url = info.skins.find((s) => s.state === State.Active)?.url}
  <Button class="h-12 w-full justify-start p-2" variant="ghost" {onclick}>
    <AccountImage {skin_url} />
    <div class="flex min-w-0 flex-1 flex-col justify-start">
      <p class="truncate text-start text-sm">{info.name}</p>
      <p class="text-muted-foreground truncate text-start text-sm">{info.id}</p>
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
    class="hover:bg-muted-foreground size-8 min-w-8"
    onclick={edit}
  >
    <Settings class="size-5!" />
  </Button>
{/snippet}
