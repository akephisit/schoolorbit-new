import { schoolShareImageResponse } from '#lib/server/school-share-image.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ fetch, url }) => schoolShareImageResponse(fetch, url.origin);
