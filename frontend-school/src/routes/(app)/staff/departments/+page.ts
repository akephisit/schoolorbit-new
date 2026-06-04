/**
 * Departments Management Page
 */

export const _meta = {
	menu: {
		title: 'โครงสร้างองค์กร',
		icon: 'Building2',
		group: 'personnel',
		order: 20,
		user_type: 'staff',
		permission: 'roles'
	}
};

export const load = async () => {
	return {
		title: 'จัดการฝ่าย'
	};
};
