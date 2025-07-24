import { redirect } from '@sveltejs/kit';

export const load = ({ url }) => {
  let id = url.searchParams.get('id');
  redirect(302, `/instances/info/logs?id=${id}`);
};
