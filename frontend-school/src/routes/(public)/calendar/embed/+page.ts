import type { PageLoad } from './$types';
import { loadPublicCalendar } from '#lib/calendar/public-route.js';

export const load: PageLoad = ({ fetch, url }) => loadPublicCalendar(fetch, url);
