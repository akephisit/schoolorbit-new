export const _meta = { access: { user_type: 'parent' } };
import type { PageLoad } from './$types';
import { appIdentityKey, waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { listChildAcademicContextOptions } from '$lib/api/academic-context';
import { resolveScopedAcademicContextUrl } from '$lib/academic-context/scoped-year';
import { listChildExamSchedules } from '$lib/api/examSchedule';
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
	const records = captureRouteLoad(
		context.then(async (result) => {
			if (!result.ok) return { ownerKey: `${appIdentityKey()}|${requestKey}`, records: [] };
			const { ownerKey, academicYearId, academicTermId } = result.data;
			if (ownerKey !== `${appIdentityKey()}|${requestKey}`) return { ownerKey, records: [] };
			return {
				ownerKey,
				records:
					academicYearId && academicTermId
						? await listChildExamSchedules(params.id, academicTermId, { requestFetch: fetch })
						: []
			};
		}),
		'โหลดตารางสอบไม่สำเร็จ'
	);
	return {
		title: 'ตารางสอบของลูก',
		requestKey,
		requestHref: url.href,
		studentId: params.id,
		context,
		records
	};
};
