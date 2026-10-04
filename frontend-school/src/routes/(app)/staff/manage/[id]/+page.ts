import { error } from '@sveltejs/kit';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { listStaffCareerHistory } from '#lib/api/staff-career.js';
import { getStaffProfile } from '#lib/api/staff.js';
import { requireApiData } from '#lib/api/client.js';
import { getAchievements } from '#lib/api/achievement.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.STAFF_PROFILE }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	depends(`school:staff-profile:${params.id}`);
	const staffId = params.id;
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(staffId))
		error(404, 'ไม่พบหน้านี้');
	const userRead = waitForAuthenticatedUser();
	const staff = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' &&
			(get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			) ||
				(user.id === staffId && get(can).has(PERMISSIONS.STAFF_PROFILE_READ_OWN)))
				? requireApiData(
						await getStaffProfile(staffId, { requestFetch: fetch }),
						'โหลดข้อมูลบุคลากรไม่สำเร็จ'
					)
				: null
		),
		'โหลดข้อมูลบุคลากรไม่สำเร็จ'
	);
	const careerHistory = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' &&
			(get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			) ||
				(user.id === staffId && get(can).has(PERMISSIONS.STAFF_PROFILE_READ_OWN)))
				? listStaffCareerHistory(staffId, {}, { requestFetch: fetch })
				: null
		),
		'โหลดประวัติไม่สำเร็จ'
	);
	const achievements = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' &&
			(get(can).has(PERMISSIONS.ACHIEVEMENT_READ_ALL) ||
				(user.id === staffId && get(can).has(PERMISSIONS.ACHIEVEMENT_READ_OWN)))
				? requireApiData(
						await getAchievements({ user_id: staffId }, { requestFetch: fetch }),
						'โหลดผลงานไม่สำเร็จ'
					)
				: []
		),
		'โหลดผลงานไม่สำเร็จ'
	);
	return { title: 'รายละเอียดบุคลากร', staffId, staff, achievements, careerHistory };
};
