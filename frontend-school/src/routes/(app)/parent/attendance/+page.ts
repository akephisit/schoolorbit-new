import type { PageLoad } from './$types';
import { currentAttendanceDate, attendanceReport } from '#lib/api/attendance.js';
import { listParentAcademicContextOptions } from '#lib/api/academic-context.js';
import { getOwnParentProfile, type ChildDto } from '#lib/api/parents.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	menu: {
		title: 'การเช็คชื่อ',
		icon: 'UserCheck',
		group: 'main',
		workspace: 'home',
		permission: PERMISSION_MODULES.ATTENDANCE,
		order: 20,
		user_type: 'parent'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const date = currentAttendanceDate();
	return {
		title: 'การเช็คชื่อ',
		date,
		initial: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				if (!user || user.user_type !== 'parent')
					throw new Error('กรุณาเข้าสู่ระบบด้วยบัญชีparent');
				const context = await listParentAcademicContextOptions(undefined, { requestFetch: fetch });
				const term =
					context.terms.find((t) => t.id === url.searchParams.get('academicTermId')) ??
					context.terms.find((t) => t.id === context.activeAcademicTermId) ??
					context.terms.at(-1);
				const children: ChildDto[] = term
					? (await getOwnParentProfile(term.academicYearId, { requestFetch: fetch })).children
					: [];
				const studentId =
					children.find((c) => c.id === url.searchParams.get('studentId'))?.id ?? children[0]?.id;
				return {
					term: term?.id ?? null,
					terms: context.terms,
					children,
					studentId: studentId ?? null,
					report:
						term && studentId
							? await attendanceReport(term.id, studentId, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดผลเช็คชื่อไม่ได้'
		)
	};
};
