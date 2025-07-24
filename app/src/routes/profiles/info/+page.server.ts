import { redirect } from '@sveltejs/kit';

export const load = ({ url }) => {
  let id = url.searchParams.get('id');
  redirect(302, `/profiles/info/quick_play?id=${id}`);
};
