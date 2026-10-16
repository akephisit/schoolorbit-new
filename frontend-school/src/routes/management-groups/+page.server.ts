import { loadPublicPersonnelDirectory } from '#lib/server/public-school.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = loadPublicPersonnelDirectory;
