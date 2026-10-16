import type { PageLoad } from './$types';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { studentRegisterContext } from '#lib/navigation/student-register.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff' as const, permission: PERMISSIONS.STUDENT_CREATE_ALL }
};
export const load: PageLoad = ({ url }) => ({
	title: 'เพิ่มทะเบียนนักเรียน',
	context: studentRegisterContext(url)
});
