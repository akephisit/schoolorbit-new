/**
 * Admission Applications List Page
 */

import { PERMISSIONS } from '#lib/permissions/registry.js';
import { redirect } from '@sveltejs/kit';
import { waitForAdmissionAccess } from '#lib/admission/admission-access.js';
import { containsProtectedIdentifier } from '#lib/admission/protected-identifier-search.js';
import { listApplications } from '#lib/api/admission.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_READ_ALL
	}
};

export const load: PageLoad = ({ depends, fetch, params, url }) => {
	depends('schoolorbit:admission-applications');
	const roundId = params.id;
	const status = url.searchParams.get('status')?.trim() ?? '';
	const search = url.searchParams.get('search')?.trim() ?? '';
	if (containsProtectedIdentifier(search)) {
		const safeUrl = new URL(url);
		safeUrl.searchParams.delete('search');
		redirect(307, `${safeUrl.pathname}${safeUrl.search}`);
	}
	const applications = captureRouteLoad(
		waitForAdmissionAccess(PERMISSIONS.ADMISSION_READ_ALL).then((allowed) =>
			allowed
				? listApplications(
						roundId,
						{ status: status || undefined, search: search || undefined },
						{ requestFetch: fetch }
					)
				: []
		),
		'โหลดใบสมัครไม่สำเร็จ'
	);
	return { title: 'ใบสมัคร', roundId, status, search, applications };
};
