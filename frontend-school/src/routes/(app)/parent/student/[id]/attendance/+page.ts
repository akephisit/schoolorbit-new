import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	access: { user_type: 'parent', permission: PERMISSIONS.ATTENDANCE_READ_OWN }
};
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listChildAcademicContextOptions } from '#lib/api/academic-context.js';
import { resolveScopedAcademicContextUrl } from '#lib/academic-context/scoped-year.js';
import { currentAttendanceDate, attendanceReport } from '#lib/api/attendance.js';
import { readAttendance } from '#lib/features/attendance/attendance-access.js';
import { getChildProfile } from '#lib/api/parents.js';
export const load: PageLoad = ({ fetch, url, depends, params }) => {
	depends('school:app-identity', 'school:attendance-own-context');
	const requestKey = url.pathname + url.search;
	const date = currentAttendanceDate();
	const context = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const ownerKey = `${appIdentityKey()}|${requestKey}`;
			const options =
				user?.user_type === 'parent' && get(can).has(PERMISSIONS.ATTENDANCE_READ_OWN)
					? await listChildAcademicContextOptions(params.id, undefined, { requestFetch: fetch })
					: null;
			const selection = options
				? resolveScopedAcademicContextUrl(options, url, true)
				: { academicYearId: '', academicTermId: '', replaceUrl: null };
			return {
				ownerKey,
				options,
				academicYearId: selection.academicYearId,
				academicTermId: selection.academicTermId,
				replaceHref: selection.replaceUrl?.href ?? null
			};
		}),
		'โหลดประวัติปีและภาคเรียนไม่สำเร็จ'
	);
	const report = captureRouteLoad(
		context.then(async (result) => {
			if (
				!result.ok ||
				result.data.ownerKey !== `${appIdentityKey()}|${requestKey}` ||
				!result.data.academicTermId
			)
				return { identityKey: appIdentityKey(), resource: null, error: null };
			return readAttendance(
				[PERMISSIONS.ATTENDANCE_READ_OWN],
				() => attendanceReport(result.data.academicTermId, params.id, { requestFetch: fetch }),
				'parent'
			);
		}),
		'โหลดผลเช็คชื่อไม่ได้'
	);
	const profile = captureRouteLoad(
		context.then(async (result) => {
			if (
				!result.ok ||
				result.data.ownerKey !== `${appIdentityKey()}|${requestKey}` ||
				!result.data.academicYearId
			)
				return { identityKey: appIdentityKey(), resource: null, error: null };
			return readAttendance(
				[PERMISSIONS.ATTENDANCE_READ_OWN],
				() => getChildProfile(params.id, result.data.academicYearId, { requestFetch: fetch }),
				'parent'
			);
		}),
		'โหลดข้อมูลนักเรียนไม่สำเร็จ'
	);

	return {
		title: 'การเช็คชื่อ',
		requestKey,
		requestHref: url.href,
		studentId: params.id,
		context,
		report,
		date,
		profile
	};
};
