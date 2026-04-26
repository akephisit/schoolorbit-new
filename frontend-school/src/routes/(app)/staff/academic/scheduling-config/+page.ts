export const _meta = {
	menu: {
		title: 'ตั้งค่าจัดตารางอัตโนมัติ',
		icon: 'Sparkles',
		group: 'academic',
		permission: 'academic_course_plan.manage.all',
		order: 75,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
