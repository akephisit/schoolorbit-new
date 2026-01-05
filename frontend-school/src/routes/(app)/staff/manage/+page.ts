/**
 * Staff Management Page
 */

export const _meta = {
	menu: {
		title: 'บุคลากร',
		icon: 'Users',
		group: 'main',
		order: 10,
		permission: 'staff'
	}
};

export const load = async () => {
	return {
		title: 'จัดการบุคลากร'
	};
};
