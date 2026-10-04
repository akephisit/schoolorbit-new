export const _meta = { access: { user_type: 'student' } };
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listMyAcademicContextOptions } from '#lib/api/academic-context.js';
import { resolveScopedAcademicYearUrl } from '#lib/academic-context/scoped-year.js';
import { getOwnProfile } from '#lib/api/students.js';
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const requestKey = url.pathname + url.search;
	const context = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const ownerKey = `${appIdentityKey()}|${requestKey}`;
			const options =
				user?.user_type === 'student'
					? await listMyAcademicContextOptions(undefined, { requestFetch: fetch })
					: null;
			const selection = options
				? resolveScopedAcademicYearUrl(options, url)
				: { academicYearId: null, replaceUrl: null };
			return {
				ownerKey,
				options,
				academicYearId: selection.academicYearId ?? '',
				replaceHref: selection.replaceUrl?.href ?? null
			};
		}),
		'โหลดประวัติปีการศึกษาไม่สำเร็จ'
	);
	const profile = captureRouteLoad(
		context.then(async (result) => {
			if (!result.ok) return { ownerKey: `${appIdentityKey()}|${requestKey}`, student: null };
			const { ownerKey, academicYearId } = result.data;
			if (ownerKey !== `${appIdentityKey()}|${requestKey}`) return { ownerKey, student: null };
			return {
				ownerKey,
				student: academicYearId
					? await getOwnProfile(academicYearId, { requestFetch: fetch })
					: null
			};
		}),
		'โหลดข้อมูลนักเรียนไม่สำเร็จ'
	);
	return { title: 'ข้อมูลส่วนตัว', requestKey, requestHref: url.href, context, profile };
};
