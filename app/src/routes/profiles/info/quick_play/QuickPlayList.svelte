<script lang="ts">
  import {
    profile_quick_play_remove,
    quick_play_server_status,
    QuickPlayType,
    type QuickPlayInfo
  } from '$lib/tauri/quick-play.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { getProfile } from '../store.svelte';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import { Play, Star, Trash2 } from '@lucide/svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import { profile_launch } from '$lib/tauri/profile.svelte';
  import { profile_favorites_set } from '$lib/tauri/home.svelte';
  import QuickPlayIcon from '$lib/components/profile/QuickPlayIcon.svelte';
  import { cn } from '@profidev/pleiades/utils';
  import { players_label } from './quick-play';

  interface Props {
    list: QuickPlayInfo[] | undefined;
    value: string;
    empty_title: string;
    empty_description: string;
  }

  let { list, value, empty_title, empty_description }: Props = $props();

  let profile = $derived(getProfile());

  let active_account = $derived(account_active.value);
  let removeOpen = $state(false);
  let remove_info = $state<QuickPlayInfo>();

  const last_played = (item: QuickPlayInfo) =>
    D.DateTime?.fromISO(item.lastPlayedTime)
      .setLocale('de')
      .toLocaleString(D.DateTime.DATETIME_SHORT);

  const launch = (item: QuickPlayInfo) =>
    profile_launch(profile!.id, profile!.name, active_account, item);
</script>

<Tabs.Content {value} class="mt-1">
  {#if list && list.length > 0}
    <div class="bg-card/50 flex flex-col overflow-hidden rounded-xl border">
      {#each list as item (item.id)}
        {@const singleplayer = item.type === QuickPlayType.Singleplayer}
        <div
          class="hover:bg-accent/40 flex items-center gap-3 border-b px-3 py-2.5 last:border-0"
        >
          <QuickPlayIcon
            profileId={profile!.id}
            quickPlay={item}
            class={cn('size-10', singleplayer && 'rounded-full')}
          />
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="truncate font-medium">{item.name}</span>
            <span class="text-muted-foreground truncate text-xs">
              {singleplayer || item.type === QuickPlayType.Realms
                ? `Last played ${last_played(item)}`
                : item.id}
            </span>
          </div>
          {#if singleplayer}
            <Button onclick={() => launch(item)}>
              <Play data-icon="inline-start" />
              Play
            </Button>
            <Button
              size="icon"
              variant="outline"
              aria-label={item.favorite
                ? 'Remove from favorites'
                : 'Add to favorites'}
              onclick={() =>
                profile_favorites_set(profile!.id, !item.favorite, item)}
            >
              <Star
                class={item.favorite ? 'fill-yellow-500 text-yellow-500' : ''}
              />
            </Button>
            <Button
              size="icon"
              variant="outline"
              aria-label="Delete world"
              onclick={() => {
                remove_info = item;
                removeOpen = true;
              }}
            >
              <Trash2 class="text-destructive" />
            </Button>
          {:else}
            {#if item.type === QuickPlayType.Multiplayer}
              {#await quick_play_server_status(item.id) then status}
                {@const label = players_label(status)}
                {#if label}
                  <span
                    class={cn(
                      'inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-xs font-medium',
                      status?.online
                        ? 'bg-green-500/15 text-green-500'
                        : 'bg-secondary text-muted-foreground'
                    )}
                  >
                    {label}
                  </span>
                {/if}
              {/await}
            {/if}
            <Button onclick={() => launch(item)}>Join</Button>
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Title>{empty_title}</Empty.Title>
        <Empty.Description>{empty_description}</Empty.Description>
      </Empty.Header>
    </Empty.Root>
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
