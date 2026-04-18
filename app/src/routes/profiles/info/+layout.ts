import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ url }) => ({
  id: url.searchParams.get('id') || ''
});
