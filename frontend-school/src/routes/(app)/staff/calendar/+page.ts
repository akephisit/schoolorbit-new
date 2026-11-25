import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { calendarGridRange } from '#lib/utils/calendar.js';
import { calendarRouteFilters } from '#lib/utils/calendar-route-filters.js';
import {
	listCalendarEvents,
	listCalendarCategories,
	listCalendarTags,
	listPendingCalendarRequests
} from '#lib/api/calendar.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ปฏิทินโรงเรียน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 7,
		user_type: 'staff',
		permission: PERMISSION_MODULES.CALENDAR
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const context = calendarRouteFilters(url);
	const access = waitForAuthenticatedUser().then((user) => {
		const permission = get(can);
		const read = user?.user_type === 'staff' && permission.has(PERMISSIONS.CALENDAR_READ_SCHOOL);
		const pending =
			read &&
			permission.hasAny(PERMISSIONS.CALENDAR_REQUEST_OWN, PERMISSIONS.CALENDAR_MANAGE_SCHOOL);
		return { read, pendingIdentity: pending ? appIdentityKey() : '' };
	});
	const events = captureRouteLoad(
		access.then(({ read }) =>
			read ? listCalendarEvents(context.filters, { requestFetch: fetch }) : []
		),
		'โหลดกิจกรรมไม่สำเร็จ'
	);
	const categories = captureRouteLoad(
		access.then(({ read }) => (read ? listCalendarCategories({ requestFetch: fetch }) : [])),
		'โหลดหมวดหมู่ไม่สำเร็จ'
	);
	const tags = captureRouteLoad(
		access.then(({ read }) => (read ? listCalendarTags({ requestFetch: fetch }) : [])),
		'โหลดแท็กไม่สำเร็จ'
	);
	const pending = access.then(async ({ pendingIdentity }) => ({
		identity: pendingIdentity,
		result: pendingIdentity
			? await captureRouteLoad(
					listPendingCalendarRequests(calendarGridRange(context.month), { requestFetch: fetch }),
					'โหลดคำร้องรออนุมัติไม่สำเร็จ'
				)
			: null
	}));
	return {
		title: _meta.menu.title,
		eventKey: context.key,
		events,
		categories,
		tags,
		pendingMonth: context.month,
		pending
	};
};
