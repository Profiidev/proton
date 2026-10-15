<script lang="ts">
  import ProfileExternalLink from '$lib/components/profile/ProfileExternalLink.svelte';
  import ProfileCard from '$lib/components/profile/ProfileCard.svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import {
    profile_favorites_list,
    profile_history_list
  } from '$lib/tauri/home.svelte';
  import {
    LoaderType,
    profile_launch,
    profile_list,
    profile_mods_count
  } from '$lib/tauri/profile.svelte';
  import { instance_list } from '$lib/tauri/instance.svelte';
  import { compareProfiles } from '$lib/util.svelte';
  import { continue_quick_play, latest_played, plural } from '$lib/profiles';
  import { running_label } from '$lib/instances';
  import { CircleArrowUp, Play } from '@lucide/svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { content_updates } from '$lib/tauri/content.svelte';
  import { updates_banner } from '$lib/content';

  let sorted_favorites = $derived(
    (profile_favorites_list.value ?? []).toSorted((a, b) =>
      compareProfiles(a.profile, b.profile)
    )
  );
  let sorted_history = $derived(
    (profile_history_list.value ?? []).toSorted((a, b) =>
      compareProfiles(a.profile, b.profile)
    )
  );
  let active_account = $derived(account_active.value);
  let current = $derived(latest_played(profile_list.value ?? []));
  let current_quick_play = $derived(
    current ? continue_quick_play(current) : undefined
  );
  let current_running = $derived(
    instance_list.value?.filter((i) => i.profile_id === current?.id).length ?? 0
  );
  let banner = $derived(
    updates_banner(content_updates.value ?? {}, profile_list.value ?? [])
  );
  let current_mods = $derived(
    current && current.loader !== LoaderType.Vanilla
      ? profile_mods_count.value?.[current.id]
      : undefined
  );
</script>

<div class="flex flex-col gap-6 px-5 pt-4 pb-6">
  {#if current}
    <section
      class="bg-card ring-foreground/10 flex flex-wrap items-center gap-4 rounded-xl p-4 ring-1"
    >
      <a
        href={`/profiles/info/quick_play/${current_quick_play?.type ?? 'singleplayer'}?id=${current.id}`}
        class="shrink-0"
      >
        <ProfileIcon
          id={current.id}
          class="bg-muted size-14 rounded-lg"
          classFallback="size-7"
        />
      </a>
      <div class="flex min-w-0 flex-[1_1_240px] flex-col gap-0.5">
        <span
          class="text-muted-foreground text-xs font-medium tracking-wider uppercase"
        >
          Continue playing
        </span>
        <h1 class="truncate text-xl font-semibold tracking-tight">
          {current_quick_play?.name ?? current.name}
        </h1>
        <span class="text-muted-foreground text-xs">
          {[
            current_quick_play && current.name,
            `${current.loader} ${current.version}`,
            current_mods !== undefined && plural(current_mods, 'mod'),
            current_running > 0 && running_label(current_running)
          ]
            .filter(Boolean)
            .join(' · ')}
        </span>
      </div>
      <Button
        onclick={() =>
          profile_launch(
            current.id,
            current.name,
            active_account,
            current_quick_play
          )}
      >
        <Play data-icon="inline-start" />
        {current_running > 0 ? 'Launch another' : 'Play'}
      </Button>
    </section>
  {/if}

  {#if banner}
    <Alert.Root class="has-data-[slot=alert-action]:pr-40">
      <CircleArrowUp />
      <Alert.Title>{banner.title}</Alert.Title>
      <Alert.Description>{banner.description}</Alert.Description>
      <Alert.Action class="top-1/2 right-3 -translate-y-1/2">
        <Button variant="outline" href={banner.href}>Review updates</Button>
      </Alert.Action>
    </Alert.Root>
  {/if}

  <section class="flex flex-col gap-2.5">
    <div class="flex items-center gap-2">
      <h2 class="text-base font-medium">Favorites</h2>
      <span
        class="bg-secondary inline-flex h-5 items-center rounded-full px-2 text-xs font-medium"
      >
        {sorted_favorites.length}
      </span>
    </div>
    {#if sorted_favorites.length > 0}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(264px,1fr))] gap-2">
        {#each sorted_favorites as favorite (`${favorite.profile.id}-${favorite.quick_play?.type}-${favorite.quick_play?.id}`)}
          <ProfileCard
            profile={favorite.profile}
            quick_play={favorite.quick_play}
          />
        {/each}
      </div>
    {:else}
      <p class="text-muted-foreground text-center">
        No favorites found. Add a profile or quick play option to your favorites
        in the profile settings.
      </p>
      <ProfileExternalLink />
    {/if}
  </section>

  <section class="flex flex-col gap-2.5">
    <div class="flex items-center gap-2">
      <h2 class="text-base font-medium">History</h2>
      <span class="text-muted-foreground text-xs">Recently played</span>
    </div>
    {#if sorted_history.length > 0}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(264px,1fr))] gap-2">
        {#each sorted_history as item (`${item.profile.id}-${item.quick_play?.type}-${item.quick_play?.id}`)}
          <ProfileCard profile={item.profile} quick_play={item.quick_play} />
        {/each}
      </div>
    {:else}
      <p class="text-muted-foreground text-center">
        No entries found in history. Launch a profile to create one.
      </p>
      <ProfileExternalLink />
    {/if}
  </section>
</div>
