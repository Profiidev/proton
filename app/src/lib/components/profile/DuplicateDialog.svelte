<script lang="ts">
  import { goto } from '$app/navigation';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { LoaderCircle } from '@lucide/svelte';
  import { ContentError, profile_duplicate } from '$lib/tauri/content.svelte';
  import { type Profile, profile_list } from '$lib/tauri/profile.svelte';

  interface Props {
    open: boolean;
    profile: Profile;
  }

  let { open = $bindable(false), profile }: Props = $props();

  let name = $state('');
  let busy = $state(false);
  // Id of the copy, opened once it shows up in the profile list
  let created = $state<string>();

  $effect(() => {
    if (open) {
      name = `${profile.name} (copy)`;
    }
  });

  $effect(() => {
    if (created && profile_list.value?.some((p) => p.id === created)) {
      const _ = goto(`/profiles/info?id=${created}`);
      created = undefined;
    }
  });

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    busy = true;
    const res = await profile_duplicate(profile.id, name.trim());
    busy = false;
    if (res in ContentError) {
      toast.error(`Failed to duplicate profile ${profile.name}`);
      return;
    }
    open = false;
    created = res;
    const _ = profile_list.update();
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Content>
    <form onsubmit={submit} class="flex flex-col gap-5">
      <Dialog.Header>
        <Dialog.Title>Duplicate profile</Dialog.Title>
        <Dialog.Description>
          Copies the profile with its content, settings and worlds.
        </Dialog.Description>
      </Dialog.Header>
      <div class="flex flex-col gap-1.5">
        <Label for="duplicate-name">Name</Label>
        <Input id="duplicate-name" bind:value={name} required />
      </div>
      <Dialog.Footer>
        <Button variant="outline" type="button" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy || !name.trim()}>
          {#if busy}
            <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {/if}
          Duplicate
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
