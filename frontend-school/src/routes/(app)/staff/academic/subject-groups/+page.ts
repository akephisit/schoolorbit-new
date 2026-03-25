export const _meta = {
	menu: {
		title: 'กลุ่มสาระการเรียนรู้',
		icon: 'GraduationCap',
		group: 'academic',
		order: 1,
		user_type: 'staff',
		permission: 'academic_curriculum'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
