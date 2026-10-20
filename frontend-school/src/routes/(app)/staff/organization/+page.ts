import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { requireApiData } from '#lib/api/client.js';
import { listOrganizationUnits, listOrganizationMembers } from '#lib/api/staff.js';
import { listSubjectGroups } from '#lib/api/academic-core.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'โครงสร้างโรงเรียน',
		icon: 'Building2',
		group: 'personnel',
		workspace: 'personnel',
		order: 20,
		user_type: 'staff',
		permission: [
			PERMISSION_MODULES.ROLES,
			PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
		]
	},
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.ROLES_READ_ALL,
			PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
		]
	}
};

export const load: PageLoad = ({ fetch, depends, url }) => {
	depends('school:app-identity', 'school:organization');
	const access = waitForAuthenticatedUser().then((user) => {
		const permissions = get(can);
		return {
			administration: user?.user_type === 'staff' && permissions.has(PERMISSIONS.ROLES_READ_ALL),
			groups:
				user?.user_type === 'staff' &&
				permissions.hasAny(
					PERMISSIONS.ROLES_READ_ALL,
					PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
					PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
				)
		};
	});
	const requestedTab = url.searchParams.get('tab');
	const tab = requestedTab === 'subject-groups' ? 'subject-groups' : 'administration';
	const units = captureRouteLoad(
		access.then(async (allowed) =>
			tab === 'administration' && allowed.administration
				? requireApiData(
						await listOrganizationUnits({ include_inactive: true }, { requestFetch: fetch }),
						'โหลดหน่วยงานไม่สำเร็จ'
					).filter((unit) => unit.unit_type !== 'subject_group')
				: []
		),
		'โหลดหน่วยงานไม่สำเร็จ'
	);
	const members = captureRouteLoad(
		units.then(async (result) => {
			if (!result.ok) throw new Error(result.error);
			const unit =
				result.data.find((unit) => unit.id === url.searchParams.get('unit')) ??
				result.data.find((unit) => unit.code === 'SCHOOL') ??
				result.data[0];
			return {
				unitId: unit?.id ?? '',
				members: unit
					? requireApiData(
							await listOrganizationMembers(unit.id, { include_children: true }, { requestFetch: fetch }),
							'โหลดสมาชิกไม่สำเร็จ'
						)
					: []
			};
		}),
		'โหลดสมาชิกไม่สำเร็จ'
	);
	const groups = captureRouteLoad(
		access.then((allowed) =>
			(tab === 'subject-groups' || !allowed.administration) && allowed.groups
				? listSubjectGroups({ requestFetch: fetch })
				: []
		),
		'โหลดกลุ่มสาระไม่สำเร็จ'
	);
	return { title: _meta.menu.title, tab, access, units, members, groups };
};
