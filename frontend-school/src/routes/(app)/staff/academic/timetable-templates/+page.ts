export const _meta = {
	menu: {
		title: 'Templates ตาราง',
		icon: 'FileStack',
		group: 'academic',
		permission: 'academic_course_plan.manage.all',
		order: 78,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
