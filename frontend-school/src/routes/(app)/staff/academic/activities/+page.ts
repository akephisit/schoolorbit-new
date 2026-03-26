/**
 * Activity Groups Page (กิจกรรมพัฒนาผู้เรียน)
 */

export const _meta = {
	menu: {
		title: 'กิจกรรมพัฒนาผู้เรียน',
		icon: 'Users',
		group: 'academic',
		order: 7,
		user_type: 'staff',
		permission: 'activity'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
