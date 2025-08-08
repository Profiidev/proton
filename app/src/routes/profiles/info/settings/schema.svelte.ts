import { z } from 'zod';

export const profileEditSchema = z.object({
  name: z.string().min(1, 'Name is required'),
  version: z.array(z.string()).min(1, 'Version is required'),
  loader_version: z.array(z.string()).min(1, 'Loader version is required'),
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
