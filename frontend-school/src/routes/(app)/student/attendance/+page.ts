import type { PageLoad } from './$types';
import { currentAttendanceDate, attendanceReport } from '#lib/api/attendance.js';
import { listMyAcademicContextOptions } from '#lib/api/academic-context.js';
import { type ChildDto } from '#lib/api/parents.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	readAttendance,
	captureAttendanceRead
} from '#lib/features/attendance/attendance-access.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	menu: {
		title: 'การเช็คชื่อ',
		icon: 'UserCheck',
		group: 'main',
		workspace: 'home',
		permission: PERMISSION_MODULES.ATTENDANCE,
		order: 20,
		user_type: 'student'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity', 'school:attendance-own-context');
	const date = currentAttendanceDate();
	return {
		title: 'การเช็คชื่อ',
		date,
		initial: captureRouteLoad(
			waitForAuthenticatedUser().then((user) => {
				const identityKey = appIdentityKey();
				return captureAttendanceRead(
					identityKey,
					Promise.resolve().then(async () => {
						if (!user || user.user_type !== 'student')
							throw new Error('กรุณาเข้าสู่ระบบด้วยบัญชีstudent');
						const context = await listMyAcademicContextOptions(undefined, { requestFetch: fetch });
						const term =
							context.terms.find((t) => t.id === url.searchParams.get('academicTermId')) ??
							context.terms.find((t) => t.id === context.activeAcademicTermId) ??
							context.terms.at(-1);
						const children: ChildDto[] = [];
						const studentId = user.id;
						return {
							term: term?.id ?? null,
							terms: context.terms,
							children,
							studentId: studentId ?? null,
							report:
								term && studentId
									? captureRouteLoad(
											readAttendance(
												[PERMISSIONS.ATTENDANCE_READ_OWN],
												() => attendanceReport(term.id, studentId, { requestFetch: fetch }),
												'student'
											),
											'โหลดผลเช็คชื่อไม่ได้'
										)
									: null
						};
					}),
					'โหลดผลเช็คชื่อไม่ได้'
				);
			}),
			'โหลดผลเช็คชื่อไม่ได้'
		)
	};
};
