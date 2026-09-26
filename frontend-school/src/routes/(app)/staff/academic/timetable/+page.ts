import type { PageLoad } from './$types';
import { selectPreferredBoardVersion } from '$lib/academic/timetable/version-selection';
import { getAcademicTermChangeSet, type AcademicTermChangeSet } from '$lib/api/learning-delivery';
import {
	getTimetableBlockWorkspace,
	listTimetableVersions,
	type TimetableBlockWorkspace
} from '$lib/api/timetable';
import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'จัดตารางสอน',
		icon: 'CalendarDays',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE,
		order: 40,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const requestedVersionId = url.searchParams.get('timetableVersionId')?.trim() || null;
	if (!academicYearId || !academicTermId) {
		return {
			title: _meta.menu.title,
			academicYearId,
			academicTermId,
			requestedVersionId,
			versions: null,
			workspace: null,
			changeSet: null
		};
	}
	const versions = captureRouteLoad(
		listTimetableVersions(academicTermId, { requestFetch: fetch }),
		'โหลดรุ่นตารางสอนไม่สำเร็จ'
	);
	const loadWorkspace = (
		versionId: string
	): Promise<RouteLoadResult<TimetableBlockWorkspace | null>> =>
		captureRouteLoad(
			getTimetableBlockWorkspace(
				{ academicYearId, academicTermId, timetableVersionId: versionId },
				{ requestFetch: fetch }
			),
			'โหลดตารางสอนไม่สำเร็จ'
		);
	const workspace = requestedVersionId
		? loadWorkspace(requestedVersionId).then(async (result) => {
				if (result.ok) return result;
				const listed = await versions;
				if (!listed.ok || listed.data.some((version) => version.id === requestedVersionId))
					return result;
				const preferred = selectPreferredBoardVersion(listed.data, null);
				return preferred
					? loadWorkspace(preferred.id)
					: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
			})
		: versions.then((result) => {
				const preferred = result.ok ? selectPreferredBoardVersion(result.data, null) : null;
				return preferred
					? loadWorkspace(preferred.id)
					: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
			});
	const changeSet: Promise<RouteLoadResult<AcademicTermChangeSet | null>> = workspace.then(
		async (result): Promise<RouteLoadResult<AcademicTermChangeSet | null>> => {
			const changeSetId = result.ok ? result.data?.version.changeSetId : null;
			return changeSetId
				? captureRouteLoad(
						getAcademicTermChangeSet(changeSetId, { requestFetch: fetch }),
						'โหลดชุดการเปลี่ยนแปลงตารางสอนไม่สำเร็จ'
					)
				: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
		}
	);
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		requestedVersionId,
		versions,
		workspace,
		changeSet
	};
};
