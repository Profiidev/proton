import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => {
  let id = url.searchParams.get('id');
  redirect(302, `/profiles/info/quick_play/singleplayer?id=${id}`);
};
