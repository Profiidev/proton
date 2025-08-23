<script lang="ts">
  import {
    profile_quick_play_remove,
    type QuickPlayInfo
  } from '$lib/tauri/quick_play.svelte';
  import {
    Button,
    Dialog,
    Separator,
    Tabs
  } from 'positron-components/components/ui';
  import { getProfile } from '../store.svelte';
  import { DateTime } from 'positron-components/util';
  import { Play, Star, Trash } from '@lucide/svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import { profile_launch } from '$lib/tauri/profile.svelte';
  import { profile_favorites_set } from '$lib/tauri/home.svelte';
  import QuickPlayIcon from '$lib/components/profile/QuickPlayIcon.svelte';

  interface Props {
    list: QuickPlayInfo[] | undefined;
    name: string;
    value: string;
    join: string;
  }

  let { list, name, join, value }: Props = $props();

  let profile = $derived(getProfile());

  let active_account = $derived(account_active.value);
  let removeOpen = $state(false);
  let remove_info = $state<QuickPlayInfo>();
</script>

<Tabs.Content {value} class="flex flex-col gap-2">
  {#if list && list.length > 0}
    {#each list as item}
      <Button variant="outline" class="h-fit w-full p-2">
        <QuickPlayIcon profileId={profile!.id} quickPlay={item} />
        <div class="ml-1 flex w-32 flex-col items-start">
          <p class="w-full truncate text-left text-base">{item.name}</p>
          <p class="text-muted-foreground w-full truncate text-left">
            {item.id}
          </p>
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
            profile_favorites_set(profile!.id, !item.favorite, item);
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
