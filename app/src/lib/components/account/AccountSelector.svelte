<script lang="ts">
  import {
    account_active,
    account_list,
    account_set_active,
    State,
    type Accounts,
    type ProfileInfo
  } from '$lib/tauri/account.svelte';
  import { CircleHelp, ExternalLink } from '@lucide/svelte';
  import { Popover, Button } from 'positron-components/components/ui';
  import AccountImage from './AccountImage.svelte';
  import { goto } from '$app/navigation';

  interface Props {
    collapsed: boolean;
  }
  let { collapsed }: Props = $props();

  let accounts: Accounts | undefined = $derived(account_list.value);
  let active: string | undefined = $derived(account_active.value);
  let open = $state(false);
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class="mt-auto ml-auto flex h-12 w-full items-center rounded-lg"
  >
    {#if active && accounts && accounts[active]}
      {@render account({
        info: accounts[active],
        onclick: () => {},
        collapsed
      })}
    {:else}
      <Button
        class="h-12 w-full cursor-pointer justify-start p-2"
        variant="ghost"
      >
        <div class="flex size-10 min-w-10 items-center justify-center">
          <CircleHelp />
        </div>
        {#if !collapsed}
          <p class="truncate text-start text-sm">No account active</p>
        {/if}
      </Button>
    {/if}
  </Popover.Trigger>
  <Popover.Content class="flex w-50 flex-col p-1">
    {#if accounts && Object.entries(accounts).length > 0}
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
      <div class="flex w-full flex-col items-center p-1">
        <p class="p-2">No Accounts</p>
        <Button
          variant="outline"
          onclick={() => goto('/settings/accounts')}
          class="text-md inline-flex w-fit cursor-pointer p-0"
          >Add One
          <ExternalLink />
        </Button>
      </div>
    {/if}
  </Popover.Content>
</Popover.Root>

{#snippet account({
  info,
  onclick,
  collapsed = false
}: {
  info: ProfileInfo;
  onclick: () => void;
  collapsed?: boolean;
})}
  {@const skin_url = info.skins.find((s) => s.state === State.Active)?.url}
  <Button
    class="h-12 w-full cursor-pointer justify-start p-2"
    variant="ghost"
    {onclick}
  >
    <AccountImage {skin_url} />
    {#if !collapsed}
      <div class="flex min-w-0 flex-1 flex-col justify-start">
        <p class="truncate text-start text-sm">{info.name}</p>
        <p class="text-muted-foreground truncate text-start text-sm">
          {info.id}
        </p>
      </div>
    {/if}
  </Button>
{/snippet}
