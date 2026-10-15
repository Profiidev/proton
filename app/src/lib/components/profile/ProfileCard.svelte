<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { DateTime as D } from '@profidev/pleiades/util/time.svelte';
  import { Play, Square, Star } from '@lucide/svelte';
  import ProfileIcon from './ProfileIcon.svelte';
  import QuickPlayIcon from './QuickPlayIcon.svelte';
  import {
    profile_launch,
    profile_mods_count,
    type Profile
  } from '$lib/tauri/profile.svelte';
  import type { QuickPlayInfo } from '$lib/tauri/quick-play.svelte';
  import { account_active } from '$lib/tauri/account.svelte';
  import { instance_list, instance_stop_all } from '$lib/tauri/instance.svelte';
  import { running_label } from '$lib/instances';
  import { card_meta } from '$lib/profiles';

  interface Props {
    profile: Profile;
    quick_play?: QuickPlayInfo;
  }

  let { profile, quick_play }: Props = $props();

  let active_account = $derived(account_active.value);
  // only profiles run, a quick play entry always offers Play
  let running = $derived(
    quick_play
      ? 0
      : (instance_list.value?.filter((i) => i.profile_id === profile.id)
          .length ?? 0)
  );
  let last_played = $derived(
    quick_play ? quick_play.lastPlayedTime : profile.last_played
  );
  let meta = $derived(
    card_meta({
      loader: profile.loader,
      mods: profile_mods_count.value?.[profile.id],
      played: last_played
        ? (D.DateTime?.fromISO(last_played).toRelative() ?? undefined)
        : undefined,
      profile_name: quick_play ? profile.name : undefined
    })
  );
  let favorite = $derived(quick_play ? quick_play.favorite : profile.favorite);
  let quickPlayIconExists = $state(false);
</script>

<div
  class="border-input bg-input/30 hover:bg-input/60 flex h-18 min-w-0 items-center gap-3 rounded-lg border p-2.5"
>
  <a
    href={`/profiles/info/quick_play/${quick_play?.type ?? 'singleplayer'}?id=${profile.id}`}
    class="flex min-w-0 flex-1 items-center gap-3"
  >
    <span class="bg-muted flex size-12 shrink-0 overflow-hidden rounded-md">
      {#if quick_play}
        <QuickPlayIcon
          profileId={profile.id}
          quickPlay={quick_play}
          bind:iconExists={quickPlayIconExists}
          noFallback
        />
      {/if}
      {#if !quickPlayIconExists}
        <ProfileIcon id={profile.id} classFallback="size-7" />
      {/if}
    </span>
    <span class="flex min-w-0 flex-1 flex-col">
      <span class="flex min-w-0 items-center gap-1.5">
        <span class="truncate text-sm leading-4.5 font-medium">
          {quick_play?.name ?? profile.name}
        </span>
        {#if favorite}
          <Star
            class="size-3 shrink-0 fill-yellow-500 text-yellow-500"
            aria-label="Favorite"
          />
        {/if}
      </span>
      <span
        class="text-muted-foreground text-[13px] leading-4 whitespace-nowrap"
      >
        {profile.loader}
        {profile.version}
      </span>
      <span class="flex h-4 min-w-0 items-center gap-1.5">
        {#if running > 0}
          <span
            class="inline-flex h-4 shrink-0 items-center gap-1 rounded-full bg-green-500/15 px-1.5 text-xs font-medium text-green-500"
          >
            <span class="size-1.5 rounded-full bg-current"></span>
            {running_label(running)}
          </span>
        {/if}
        <span class="text-muted-foreground truncate text-xs">{meta}</span>
      </span>
    </span>
  </a>
  {#if running > 0}
    <Button
      size="icon"
      variant="destructive"
      aria-label="Stop instances"
      onclick={() => instance_stop_all(profile.id)}
    >
      <Square />
    </Button>
  {:else}
    <Button
      size="icon"
      aria-label="Play profile"
      onclick={() =>
        profile_launch(profile.id, profile.name, active_account, quick_play)}
    >
      <Play />
    </Button>
  {/if}
</div>
