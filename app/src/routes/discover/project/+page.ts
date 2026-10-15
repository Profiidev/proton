import type { ContentKind, Source } from '$lib/tauri/content.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = ({
  url
}): { id: string; kind: ContentKind; source: Source } => ({
  id: url.searchParams.get('id') ?? '',
  // oxlint-disable-next-line no-unsafe-type-assertion
  kind: (url.searchParams.get('kind') ?? 'mod') as ContentKind,
  // oxlint-disable-next-line no-unsafe-type-assertion
  source: (url.searchParams.get('source') ?? 'modrinth') as Source
});
