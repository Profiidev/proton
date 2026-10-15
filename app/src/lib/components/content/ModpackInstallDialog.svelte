<script lang="ts">
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import {
    CircleAlert,
    Info,
    LoaderCircle,
    TriangleAlert
  } from '@lucide/svelte';
  import VersionSelect from '$lib/components/form/VersionSelect.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import ManualDownloads from './ManualDownloads.svelte';
  import { plural } from '$lib/profiles';
  import { account_active } from '$lib/tauri/account.svelte';
  import { settings_get } from '$lib/tauri/settings.svelte';
  import { profile_launch } from '$lib/tauri/profile.svelte';
  import {
    ContentError,
    content_versions,
    modpack_install,
    modpack_plan,
    type PackPlan,
    type ProjectSummary,
    type VersionInfo
  } from '$lib/tauri/content.svelte';
  import { default_version, memory_fits, version_label } from './install';

  interface Props {
    open?: boolean;
    /** Pack to install, its versions are listed */
    project?: ProjectSummary;
    /** Precomputed plan, e.g. from an imported file */
    plan?: PackPlan;
  }

  let { open = $bindable(false), project, plan }: Props = $props();

  let versions = $state<VersionInfo[]>([]);
  let picked = $state<string>();
  let version = $derived(
    versions.find((v) => v.id === picked) ?? default_version(versions)
  );
  let version_id = $derived(version?.id);

  $effect(() => {
    versions = [];
    if (!open || !project) return;
    const { source, id } = project;
    let stale = false;
    void content_versions(source, id).then((list = []) => {
      if (stale) return;
      versions = list;
    });
    return () => {
      stale = true;
    };
  });

  let remote = $state<PackPlan>();
  let remote_error = $state<ContentError>();

  $effect(() => {
    remote = undefined;
    remote_error = undefined;
    if (!project || !version_id) return;
    let stale = false;
    void modpack_plan({
      projectId: project.id,
      source: project.source,
      type: 'remote',
      versionId: version_id
    }).then((result) => {
      if (stale) return;
      if (typeof result === 'string') {
        remote_error = result;
      } else {
        remote = result;
      }
    });
    return () => {
      stale = true;
    };
  });

  let current = $derived(plan ?? remote);
  let name = $derived(current?.name ?? project?.name ?? '');
  let start = $state(false);

  let max_mem = $derived(settings_get.value?.system_max_mem);
  let memory_ok = $derived(
    current?.recommendedMemoryMb === undefined ||
      memory_fits(current.recommendedMemoryMb, max_mem)
  );
  let chips = $derived(
    (
      [
        [current?.counts.mods ?? 0, 'mod'],
        [current?.counts.resourcepacks ?? 0, 'resource pack'],
        [current?.counts.shaders ?? 0, 'shader']
      ] as const
    )
      .filter(([n]) => n > 0)
      .map(([n, word]) => plural(n, word))
  );

  const gb = (mb: number) => `${(mb / 1024).toFixed(1)} GB`;

  let found = $state<Record<string, string>>({});
  let manual_done = $derived(
    current?.manual.every((f) => f.fileId in found) ?? false
  );

  const install = async () => {
    // closing resets the reactive plan, keep what this install needs
    const [pack, paths] = [current, found];
    if (!pack) return;
    const profile_name = name.trim();
    open = false;
    const id = await modpack_install(pack, profile_name, paths);
    if (id && start) {
      await profile_launch(id, profile_name, account_active.value);
    }
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="max-h-[calc(100vh-32px)] w-[min(540px,calc(100vw-32px))] max-w-none gap-4.5 overflow-y-auto sm:max-w-none"
  >
    <Dialog.Header>
      <Dialog.Title>Install {current?.name ?? project?.name}</Dialog.Title>
      <Dialog.Description>
        This creates a new profile. Your other profiles are not touched.
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex items-center gap-3.5">
      <ProjectIcon
        url={current?.iconUrl ?? project?.iconUrl}
        {name}
        class="size-16"
      />
      <div class="flex flex-col gap-1.5">
        {#if current}
          <span class="text-muted-foreground">
            {current.loader} · Minecraft {current.minecraft}
          </span>
          <div class="flex flex-wrap gap-1.5">
            {#each chips as chip (chip)}
              <span
                class="bg-secondary inline-flex h-5 items-center rounded-full px-2 text-xs font-medium"
              >
                {chip}
              </span>
            {/each}
          </div>
        {:else if remote_error}
          <span class="text-destructive text-sm">
            {remote_error === ContentError.InvalidPack
              ? 'This pack version can not be read.'
              : 'Failed to load the pack, check your connection.'}
          </span>
        {:else}
          <span class="text-muted-foreground flex items-center gap-2 text-sm">
            <LoaderCircle class="size-4 animate-spin" />
            Loading pack
          </span>
        {/if}
      </div>
    </div>

    <div class="flex flex-wrap gap-3">
      <div class="flex min-w-0 flex-[1.4_1_200px] flex-col gap-1.5">
        <Label for="modpack-name">Profile name</Label>
        <Input id="modpack-name" bind:value={name} />
      </div>
      {#if project}
        <div class="flex min-w-0 flex-[1_1_160px] flex-col gap-1.5">
          <Label>Pack version</Label>
          <VersionSelect
            label="Pack version"
            options={versions.map(version_label)}
            disabled={versions.length === 0}
            bind:value={
              () => version && version_label(version),
              (l) => (picked = versions.find((v) => version_label(v) === l)?.id)
            }
          />
        </div>
      {/if}
    </div>

    {#if current?.recommendedMemoryMb !== undefined}
      {#if memory_ok}
        <Alert.Root>
          <Info />
          <Alert.Title>Memory is set up for you</Alert.Title>
          <Alert.Description>
            The pack asks for {gb(current.recommendedMemoryMb)}.
            {#if max_mem !== undefined}
              Your system has {gb(max_mem)}, so nothing needs changing.
            {/if}
          </Alert.Description>
        </Alert.Root>
      {:else}
        <Alert.Root
          class="text-yellow-500 *:data-[slot=alert-description]:text-yellow-500/90"
        >
          <TriangleAlert />
          <Alert.Title>Not enough memory for this pack</Alert.Title>
          <Alert.Description>
            The pack asks for {gb(current.recommendedMemoryMb)}, but your system
            only has {gb(max_mem ?? 0)}. The game may run slowly or crash.
          </Alert.Description>
        </Alert.Root>
      {/if}
    {/if}

    {#if current && current.manual.length > 0}
      <Alert.Root>
        <CircleAlert />
        <Alert.Title>
          {plural(current.manual.length, 'file')} need a manual download
        </Alert.Title>
        <Alert.Description class="flex flex-col gap-2">
          <span>
            CurseForge only allows these from its website. Download each one, it
            is picked up automatically.
          </span>
          <ManualDownloads files={current.manual} bind:found />
        </Alert.Description>
      </Alert.Root>
    {/if}

    <label class="flex items-center gap-2 text-sm">
      <Checkbox bind:checked={start} />
      Start the game when the download finishes
    </label>

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
      <Button
        disabled={!current || name.trim() === '' || !manual_done}
        onclick={install}
      >
        Create profile and install
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
