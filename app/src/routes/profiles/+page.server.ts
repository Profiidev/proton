import { superValidate } from 'sveltekit-superforms';
import type { PageServerLoad } from './$types';
import { zod } from 'sveltekit-superforms/adapters';
import { profileCreateSchema } from './schema.svelte';

export const load: PageServerLoad = async () => {
  return {
    profileCreate: await superValidate(zod(profileCreateSchema))
  };
};
