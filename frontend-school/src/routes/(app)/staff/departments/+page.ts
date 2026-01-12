/**
 * Departments Management Page
 */

export const _meta = {
	menu: {
		title: 'จัดการฝ่าย',
		icon: 'Building2',
		group: 'settings',
		order: 1002,
		user_type: 'staff',
		permission: 'departments'
	}
};

export const load = async () => {
	return {
		title: 'จัดการฝ่าย'
	};
};
