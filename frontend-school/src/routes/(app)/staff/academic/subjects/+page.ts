/**
 * Subject Management Page
 */

export const _meta = {
	menu: {
		title: 'คลังรายวิชา',
		icon: 'BookOpen',
		group: 'academic',
		order: 2,
		user_type: 'staff',
		permission: 'academic_curriculum'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
