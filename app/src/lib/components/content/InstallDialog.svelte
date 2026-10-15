<script lang="ts">
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import * as RadioGroup from '@profidev/pleiades/components/ui/radio-group';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { cn } from '@profidev/pleiades/utils';
  import { CircleAlert, Download, LoaderCircle } from '@lucide/svelte';
  import VersionSelect from '$lib/components/form/VersionSelect.svelte';
  import ProfileIcon from '$lib/components/profile/ProfileIcon.svelte';
  import ManualDownloads from './ManualDownloads.svelte';
  import { profile_compat } from '$lib/content';
  import { content_target } from '$lib/content-target.svelte';
  import { profile_list } from '$lib/tauri/profile.svelte';
  import {
    ContentError,
    content_adopt_file,
    content_install,
    content_plan_install,
    content_versions,
    type InstallPlan,
    type ProjectSummary,
    type VersionInfo
  } from '$lib/tauri/content.svelte';
  import { default_version, profile_note, version_label } from './install';

  interface Props {
    open?: boolean;
    project: ProjectSummary;
    versionId?: string;
  }

  let { open = $bindable(false), project, versionId }: Props = $props();

  const chip =
    'inline-flex h-5 items-center gap-1 rounded-full px-2 text-xs font-medium';

  let tiles = $derived(
    (profile_list.value ?? []).map((profile) => {
      const compat = profile_compat(profile, project.kind, project);
      return { compat, note: profile_note(profile, compat), profile };
    })
  );

  let query = $state('');
  let shown = $derived(
    tiles.filter((t) =>
      t.profile.name.toLowerCase().includes(query.trim().toLowerCase())
    )
  );

  // Picked by the user, else the target, else the first compatible profile
  let chosen = $state<string>();
  let profile = $derived.by(() => {
    const ok = tiles.filter((t) => t.compat.ok).map((t) => t.profile);
    return (
      ok.find((p) => p.id === chosen) ??
      ok.find((p) => p.id === content_target.id) ??
      ok[0]
    );
  });

  let versions = $state<VersionInfo[]>([]);
  let label = $state<string>();
  let version = $derived(
    versions.find((v) => version_label(v) === label) ??
      default_version(versions, versionId)
  );

  $effect(() => {
    const id = profile?.id;
    versions = [];
    if (!open || !id) return;
    let stale = false;
    void content_versions(project.source, project.id, id).then((list = []) => {
      if (stale) return;
      versions = list;
    });
    return () => {
      stale = true;
    };
  });

  let plan = $state<InstallPlan>();
  let plan_error = $state<ContentError>();
  let found = $state<Record<string, string>>({});

  $effect(() => {
    const id = profile?.id;
    const version_id = version?.id;
    plan = undefined;
    plan_error = undefined;
    if (!id || !version_id) return;
    let stale = false;
    void content_plan_install(id, [
      { projectId: project.id, source: project.source, versionId: version_id }
    ]).then((result) => {
      if (stale) return;
      if (typeof result === 'string') {
        plan_error = result;
      } else {
        plan = result;
      }
    });
    return () => {
      stale = true;
    };
  });

  let dependencies = $derived(
    plan?.items.filter((i) => i.reason === 'dependency') ?? []
  );
  let manual_done = $derived(
    plan?.manual.every((f) => f.fileId in found) ?? false
  );
  let can_install = $derived(
    plan !== undefined && plan.conflicts.length === 0 && manual_done
  );

  const install = async () => {
    // closing resets the reactive plan, keep what this install needs
    const [target, picked, paths] = [profile, plan, found];
    if (!picked || !target) return;
    open = false;
    content_target.id = target.id;
    // Restricted files come from the download folders
    for (const file of picked.manual) {
      if (await content_adopt_file(target.id, file, paths[file.fileId])) {
        toast.error(`Failed to add ${file.fileName} to ${target.name}`);
        return;
      }
    }
    const rest = { ...picked, manual: [] };
    if (rest.items.length === 0) {
      toast.success(`Installed ${project.name} in ${target.name}`);
    } else {
      void content_install(rest, target.name);
    }
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="max-h-[calc(100vh-32px)] w-[min(520px,calc(100vw-32px))] max-w-none gap-4.5 overflow-y-auto sm:max-w-none"
  >
    <Dialog.Header>
      <Dialog.Title>Install {project.name}</Dialog.Title>
      <Dialog.Description>
        Choose which profile gets this {project.kind === 'mod'
          ? 'mod'
          : 'pack'}. Only compatible profiles can be selected.
      </Dialog.Description>
    </Dialog.Header>

    <Input placeholder="Search profiles" bind:value={query} />
    <RadioGroup.Root
      bind:value={() => profile?.id ?? '', (id) => (chosen = id)}
      aria-label="Profile"
      class="max-h-72 shrink-0 gap-2 overflow-y-auto"
    >
      {#each shown as { profile: p, compat, note } (p.id)}
        <label
          class={cn(
            'border-input bg-input/30 hover:bg-input/60 flex items-center gap-3 rounded-lg border px-3 py-2.5',
            !compat.ok && 'pointer-events-none opacity-50'
          )}
        >
          <RadioGroup.Item value={p.id} disabled={!compat.ok} />
          <ProfileIcon id={p.id} class="size-9" classFallback="size-5" />
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate font-medium">{p.name}</span>
            <span class="text-muted-foreground text-xs">{note}</span>
          </span>
        </label>
      {:else}
        <p class="text-muted-foreground text-sm">
          {tiles.length === 0 ? 'No profiles yet.' : 'No matching profiles.'}
        </p>
      {/each}
    </RadioGroup.Root>

    <div class="flex flex-col gap-1.5">
      <Label>Version</Label>
      <VersionSelect
        label="Version"
        options={versions.map(version_label)}
        disabled={versions.length === 0}
        bind:value={() => version && version_label(version), (l) => (label = l)}
      />
    </div>

    {#if plan_error}
      <Alert.Root variant="destructive">
        <CircleAlert />
        <Alert.Title>
          {plan_error === ContentError.Incompatible
            ? 'Not compatible with this profile'
            : 'Could not resolve dependencies'}
        </Alert.Title>
        <Alert.Description>
          {plan_error === ContentError.Incompatible
            ? 'This version can not run with the loader or Minecraft version of the profile.'
            : 'Check your connection and try again.'}
        </Alert.Description>
      </Alert.Root>
    {:else if version && !plan}
      <div class="text-muted-foreground flex items-center gap-2 text-sm">
        <LoaderCircle class="size-4 animate-spin" />
        Checking dependencies
      </div>
    {/if}

    {#if plan}
      {#if dependencies.length > 0 || plan.alreadyInstalled.length > 0}
        <div
          class="bg-muted/40 ring-foreground/10 flex flex-col gap-2 rounded-xl px-3.5 py-3 ring-1"
        >
          <span
            class="text-muted-foreground text-xs font-medium tracking-wider uppercase"
          >
            Also needed
          </span>
          {#each plan.alreadyInstalled as dep (dep.projectId)}
            <div class="flex items-center justify-between gap-2 text-sm">
              <span class="truncate">{dep.name}</span>
              <span class={cn(chip, 'bg-green-500/15 text-green-500')}
                >Already installed</span
              >
            </div>
          {/each}
          {#each dependencies as dep (dep.version.id)}
            <div class="flex items-center justify-between gap-2 text-sm">
              <span class="truncate">{dep.name}</span>
              <span class={cn(chip, 'bg-secondary')}>
                Will install {dep.version.versionNumber}
              </span>
            </div>
          {/each}
        </div>
      {/if}

      {#if plan.optional.length > 0}
        <p class="text-muted-foreground text-xs">
          Optional: {plan.optional.map((o) => o.name).join(', ')}
        </p>
      {/if}

      {#if plan.conflicts.length > 0}
        <Alert.Root variant="destructive">
          <CircleAlert />
          <Alert.Title>Conflicts with installed content</Alert.Title>
          <Alert.Description>
            {#each plan.conflicts as conflict (conflict.projectId)}
              <span>{conflict.name}: {conflict.reason}</span>
            {/each}
          </Alert.Description>
        </Alert.Root>
      {/if}

      {#if plan.manual.length > 0 && profile}
        <div class="flex flex-col gap-1.5">
          <span class="text-sm">
            CurseForge only allows these files to be downloaded from its
            website. Download each one, it is picked up automatically.
          </span>
          <ManualDownloads files={plan.manual} bind:found />
        </div>
      {/if}
    {/if}

    <Dialog.Footer>
      <Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
      <Button disabled={!can_install} onclick={install}>
        <Download />
        Install to {profile?.name ?? 'profile'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
