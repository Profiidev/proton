<script lang="ts">
  import type { Component } from 'svelte';
  import { page } from '$app/state';
  import { crossfade } from 'svelte/transition';
  import { cubicInOut } from 'svelte/easing';
  import { cn } from '@profidev/pleiades/utils';

  interface Props {
    class?: string;
    items: {
      href: string;
      title: string;
      icon?: Component<{ class?: string }>;
      /** Path prefix that marks the item active, defaults to the href path */
      match?: string;
      /** Amber count chip, hidden when 0 */
      badge?: number;
    }[];
  }

  const { class: className = undefined, items }: Props = $props();

  const [send, receive] = crossfade({
    duration: 250,
    easing: cubicInOut
  });
</script>

<nav class={cn('flex flex-col gap-0.5', className)}>
  {#each items as item (item.href)}
    {@const url = new URL(item.href, location.origin)}
    {@const isActive =
      page.url.pathname.startsWith(item.match ?? url.pathname) &&
      page.url.search === url.search}

    <a
      href={item.href}
      aria-current={isActive ? 'page' : undefined}
      class={cn(
        'relative flex h-8 items-center gap-2 rounded-md px-2.5 text-sm font-medium whitespace-nowrap',
        isActive
          ? 'text-foreground'
          : 'text-muted-foreground hover:bg-accent/60 hover:text-foreground'
      )}
      data-sveltekit-noscroll
    >
      {#if isActive}
        <div
          class="bg-muted absolute inset-0 rounded-md"
          in:send={{ key: 'active-sidebar-tab' }}
          out:receive={{ key: 'active-sidebar-tab' }}
        ></div>
      {/if}
      {#if item.icon}
        <item.icon class="relative size-4 shrink-0" />
      {/if}
      <span class="relative">{item.title}</span>
      {#if item.badge}
        <span
          class="relative ml-auto inline-flex h-5 items-center rounded-full bg-amber-500/15 px-1.5 text-xs font-medium text-amber-500"
        >
          {item.badge}
        </span>
      {/if}
    </a>
  {/each}
</nav>
