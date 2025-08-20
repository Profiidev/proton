import { superValidate } from 'sveltekit-superforms';
import type { PageServerLoad } from './$types';
import { zod } from 'sveltekit-superforms/adapters';
import { profileEditSchema } from './schema.svelte';

export const load: PageServerLoad = async () => {
  return {
    profileEdit: await superValidate(zod(profileEditSchema))
  };
};
