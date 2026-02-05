/**
 * Staff Achievements Management Page
 */

export const ssr = false;

export const _meta = {
	menu: {
		title: 'บันทึกเกียรติบัตร',
		icon: 'Award',
		group: 'personnel',
		order: 30, // After "Manage Staff" (10)
		user_type: 'staff',
		permission: 'achievement.read.all'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
