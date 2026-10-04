export const _meta = { access: { user_type: 'parent' } };
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listChildAcademicContextOptions } from '#lib/api/academic-context.js';
import { resolveScopedAcademicYearUrl } from '#lib/academic-context/scoped-year.js';
import { getChildProfile } from '#lib/api/parents.js';
export const load: PageLoad = ({ fetch, url, depends, params }) => {
	depends('school:app-identity');
	const requestKey = url.pathname + url.search;
	const context = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const ownerKey = `${appIdentityKey()}|${requestKey}`;
			const options =
				user?.user_type === 'parent'
					? await listChildAcademicContextOptions(params.id, undefined, { requestFetch: fetch })
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
					? await getChildProfile(params.id, academicYearId, { requestFetch: fetch })
					: null
			};
		}),
		'โหลดข้อมูลนักเรียนไม่สำเร็จ'
	);
	return {
		title: 'ข้อมูลนักเรียน',
		requestKey,
		requestHref: url.href,
		studentId: params.id,
		context,
		profile
	};
};
