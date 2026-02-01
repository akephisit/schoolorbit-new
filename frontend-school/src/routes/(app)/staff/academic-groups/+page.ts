/**
 * Departments Management Page
 */

export const _meta = {
	menu: {
		title: 'จัดการกลุ่มสาระ',
		icon: 'Building2',
		group: 'settings',
		order: 1002,
		user_type: 'staff',
		permission: 'departments'
	}
};

export const load = async () => {
	return {
		title: 'จัดการกลุ่มสาระ'
	};
};
