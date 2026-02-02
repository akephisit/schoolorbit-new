/**
 * Staff Management Page
 */

export const _meta = {
	menu: {
		title: 'บุคลากร',
		icon: 'Users',
		group: 'personnel',
		order: 10,
		user_type: 'staff',
		permission: 'staff'
	}
};

export const load = async () => {
	return {
		title: 'จัดการบุคลากร'
	};
};
