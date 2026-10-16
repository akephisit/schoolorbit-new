import { schoolIconResponse } from '#lib/server/school-icon.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ fetch, url }) => schoolIconResponse(fetch, url.origin);
