<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import {
    profile_favorites_set,
    profile_launch,
    profile_quick_play_list,
    profile_quick_play_remove,
    QuickPlayType,
    type QuickPlayInfo
  } from '$lib/tauri/profile.svelte';
  import {
    Button,
    Dialog,
    Separator,
    Tabs
  } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { DateTime } from 'positron-components/util';
  import { Play, Star, Trash } from '@lucide/svelte';
  import { compareDateTimes } from '$lib/util.svelte';
  import { account_active } from '$lib/tauri/account.svelte';

  let profile = $derived(getProfile());

  let active_account = $derived(account_active.value);
  let singleplayer = $state<QuickPlayInfo[] | undefined>();
  let multiplayer = $state<QuickPlayInfo[] | undefined>();
  let realms = $state<QuickPlayInfo[] | undefined>();
  let removeOpen = $state(false);
  let remove_info = $state<QuickPlayInfo>();

  let quick_play_updater = $derived(
    profile &&
      create_data_state(async () => {
        let quick_play_list = await profile_quick_play_list(profile.id);

        singleplayer = quick_play_list
          ?.filter((q) => q.type === QuickPlayType.Singleplayer)
          .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

        multiplayer = quick_play_list
          ?.filter((q) => q.type === QuickPlayType.Multiplayer)
          .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

        realms = quick_play_list
          ?.filter((q) => q.type === QuickPlayType.Realms)
          .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

        return quick_play_list;
      }, UpdateType.ProfileQuickPlay)
  );
  let x = $derived(quick_play_updater?.value);
</script>

<!-- we need to use x so the updater is active or the icon will not be updated -->
<p class="absolute top-1000 left-1000 hidden">{x}</p>
<div class="flex w-full flex-col gap-2">
  <Tabs.Root
    value={singleplayer?.length
      ? 'singleplayer'
      : multiplayer?.length
        ? 'multiplayer'
        : realms?.length
          ? 'realms'
          : 'singleplayer'}
  >
    <Tabs.List>
      <Tabs.Trigger value="singleplayer">Singleplayer</Tabs.Trigger>
      <Tabs.Trigger value="multiplayer">Multiplayer</Tabs.Trigger>
      <Tabs.Trigger value="realms">Realms</Tabs.Trigger>
    </Tabs.List>

    {@render tab({
      list: singleplayer,
      value: 'singleplayer',
      name: 'singleplayer',
      join: 'singleplayer world'
    })}
    {@render tab({
      list: multiplayer,
      value: 'multiplayer',
      name: 'multiplayer',
      join: 'server'
    })}
    {@render tab({
      list: realms,
      value: 'realms',
      name: 'realms',
      join: 'realm'
    })}
  </Tabs.Root>
</div>

{#snippet tab({
  list,
  value,
  name,
  join
}: {
  list: QuickPlayInfo[] | undefined;
  value: string;
  name: string;
  join: string;
})}
  <Tabs.Content {value} class="flex flex-col gap-2">
    {#if list && list.length > 0}
      {#each list as item}
        <Button variant="outline" class="h-fit w-full p-2">
          <div class="ml-2 flex flex-col items-start">
            <p class="text-base">{item.name}</p>
            <p class="text-muted-foreground">{item.id}</p>
          </div>
          <Separator orientation="vertical" class="mx-1 h-8!" />
          <p class="text-muted-foreground">
            Last played at {DateTime.fromISO(item.lastPlayedTime)
              .setLocale('de')
              .toLocaleString(DateTime.DATETIME_SHORT)}
          </p>
          <Button
            class="ml-auto cursor-pointer"
            onclick={(e) => {
              e.stopImmediatePropagation();
              profile_launch(profile!.id, profile!.name, active_account, item);
            }}
          >
            <Play />
            Play
          </Button>
          <Button
            size="icon"
            variant="outline"
            onclick={() => {
              profile_favorites_set(profile!.id, !profile!.favorite, item);
            }}
            class="cursor-pointer"
          >
            <Star
              class={item.favorite
                ? 'fill-yellow-500 text-yellow-500'
                : 'text-muted-foreground'}
            />
          </Button>
          <Button
            variant="destructive"
            class="cursor-pointer"
            size="icon"
            onclick={(e) => {
              e.stopImmediatePropagation();
              remove_info = item;
              removeOpen = true;
            }}
          >
            <Trash />
          </Button>
        </Button>
      {/each}
    {:else}
      <p class="text-muted-foreground mt-2 ml-2">
        No quick play options available for {name}. Join a {join}
        to add it to quick play.
      </p>
      <p class="text-muted-foreground ml-2 text-sm">
        Minecraft versions before 1.20 (23w14a) do not support quick play.
      </p>
    {/if}
  </Tabs.Content>
{/snippet}
<Dialog.Root bind:open={removeOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Remove Entry</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to remove the quick play entry {remove_info?.name}?
      </Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button
        type="submit"
        variant="destructive"
        onclick={() => {
          remove_info && profile_quick_play_remove(profile!.id, remove_info);
          removeOpen = false;
        }}
      >
        Remove
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
