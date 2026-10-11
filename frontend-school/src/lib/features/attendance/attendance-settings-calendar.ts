import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import {
	listCalendarEvents,
	listMyCalendarEvents,
	type CalendarViewerEvent
} from '#lib/api/calendar.js';
import type { ApiRequestOptions } from '#lib/api/client.js';
import { calendarGridRange } from '#lib/utils/calendar.js';

export type AttendanceCalendarEvents = { events: CalendarViewerEvent[]; scope: 'school' | 'own' };
export async function attendanceSettingsCalendar(
	month: string,
	options: ApiRequestOptions = {}
): Promise<AttendanceCalendarEvents> {
	const scope = get(can).has(PERMISSIONS.CALENDAR_READ_SCHOOL) ? 'school' : 'own';
	const range = calendarGridRange(month + '-01');
	const events =
		scope === 'school'
			? await listCalendarEvents(range, options)
			: await listMyCalendarEvents(range, options);
	return { scope, events };
}
export function attendanceMonthRange(month: string) {
	const [year, number] = month.split('-').map(Number);
	return {
		from: month + '-01',
		to: new Date(Date.UTC(year, number, 0)).toISOString().slice(0, 10)
	};
}
