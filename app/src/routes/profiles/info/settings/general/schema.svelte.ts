import { LoaderType, ModdedLoaderType } from '$lib/tauri/profile.svelte';
import { z } from 'zod';

const baseProfileEditSchema = z.object({
  name: z.string().min(1, 'Name is required'),
  version: z.array(z.string()).min(1, 'Version is required'),
  icon: z
    .instanceof(File, {
      message: 'Icon must be a file'
    })
    .refine(
      (file) => ['image/png', 'image/jpeg', 'image/jpg'].includes(file.type),
      { message: 'Invalid image file type' }
    )
    .optional()
});

const vanillaProfileEditSchema = z.object({
  loader: z.literal(LoaderType.Vanilla)
});

const moddedProfileEditSchema = z.object({
  loader: z.nativeEnum(ModdedLoaderType),
  loader_version: z.array(z.string()).min(1, 'Loader version is required')
});

export const profileEditSchema = z
  .discriminatedUnion('loader', [
    vanillaProfileEditSchema,
    moddedProfileEditSchema
  ])
  .and(baseProfileEditSchema);
