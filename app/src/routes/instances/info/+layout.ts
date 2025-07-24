import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ url }) => {
  return {
    id: url.searchParams.get('id') || ''
  };
};
