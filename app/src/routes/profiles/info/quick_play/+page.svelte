<script lang="ts">
  import { create_data_state, UpdateType } from '$lib/data_state.svelte';
  import {
    profile_quick_play_list,
    QuickPlayType,
    type QuickPlayInfo
  } from '$lib/tauri/profile.svelte';
  import { Button, Separator, Tabs } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { DateTime } from 'positron-components/util';
  import { Play } from '@lucide/svelte';
  import { compareDateTimes } from '$lib/util.svelte';

  let profile = $derived(getProfile());

  let quick_play_updater = $derived(
    profile &&
      create_data_state(async () => {
        return await profile_quick_play_list(profile.id);
      }, UpdateType.ProfileQuickPlay)
  );
  let quick_play_list = $derived(quick_play_updater?.value);

  let singleplayer = $derived(
    quick_play_list
      ?.filter((q) => q.type === QuickPlayType.Singleplayer)
      .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime))
  );
  let multiplayer = $derived(
    quick_play_list
      ?.filter((q) => q.type === QuickPlayType.Multiplayer)
      .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime))
  );
  let realms = $derived(
    quick_play_list
      ?.filter((q) => q.type === QuickPlayType.Realms)
      .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime))
  );
</script>

<div class="flex w-full flex-col gap-2">
  {#if !singleplayer?.length && !multiplayer?.length && !realms?.length}
    <p class="text-muted-foreground mt-2 ml-2">
      No quick play options available for this profile. Join a server or world
      to add it to quick play.
    </p>
    <p class="text-muted-foreground ml-2 text-sm">
      Minecraft versions before 1.20 (23w14a) do not support quick play.
    </p>
  {:else}
    <Tabs.Root
      value={singleplayer?.length
        ? 'singleplayer'
        : multiplayer?.length
          ? 'multiplayer'
          : 'realms'}
    >
      <Tabs.List>
        {#if (singleplayer?.length || 0) > 0}
          <Tabs.Trigger value="singleplayer">Singleplayer</Tabs.Trigger>
        {/if}
        {#if (multiplayer?.length || 0) > 0}
          <Tabs.Trigger value="multiplayer">Multiplayer</Tabs.Trigger>
        {/if}
        {#if (realms?.length || 0) > 0}
          <Tabs.Trigger value="realms">Realms</Tabs.Trigger>
        {/if}
      </Tabs.List>

      {@render tab({
        list: singleplayer,
        value: 'singleplayer'
      })}
      {@render tab({
        list: multiplayer,
        value: 'multiplayer'
      })}
      {@render tab({
        list: realms,
        value: 'realms'
      })}
    </Tabs.Root>
  {/if}
</div>

{#snippet tab({
  list,
  value
}: {
  list: QuickPlayInfo[] | undefined;
  value: string;
})}
  <Tabs.Content {value} class="flex flex-col gap-2">
    {#if list}
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
            }}
          >
            <Play />
            Play
          </Button>
        </Button>
      {/each}
    {/if}
  </Tabs.Content>
{/snippet}
