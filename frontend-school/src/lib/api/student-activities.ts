import { apiClient, requireApiData } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type StudentActivityOffering = Schemas['StudentActivityOfferingOption'];
export type StudentActivityGroup = Schemas['StudentActivityGroupOption'];
export type StudentActivityRegistrationResult = Schemas['StudentActivityRegistrationResult'];
export type StudentActivityRegistrationFilters =
	operations['listMyActivityRegistrations']['parameters']['query'];

const activityTypeLabels: Record<string, string> = {
	scout: 'ลูกเสือ / เนตรนารี / ยุวกาชาด',
	club: 'ชุมนุม',
	guidance: 'แนะแนว',
	social: 'กิจกรรมเพื่อสังคม',
	other: 'กิจกรรมอื่น ๆ'
};

export function getStudentActivityTypeLabel(activityType: string): string {
	return activityTypeLabels[activityType] ?? activityType;
}

function registrationPath(academicTermId: string, groupId?: string): string {
	const selectedTermId = academicTermId.trim();
	if (!selectedTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	const path = groupId
		? `/api/me/activity-registrations/${encodeURIComponent(groupId)}`
		: '/api/me/activity-registrations';
	return `${path}?academicTermId=${encodeURIComponent(selectedTermId)}`;
}

export async function listMyActivityRegistrations(
	filters: StudentActivityRegistrationFilters,
	signal?: AbortSignal
): Promise<StudentActivityOffering[]> {
	const response = await apiClient.get<StudentActivityOffering[]>(
		registrationPath(filters.academicTermId),
		{ signal }
	);
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมที่เปิดลงทะเบียนได้');
}

export async function enrollMyActivityRegistration(
	academicTermId: string,
	groupId: string
): Promise<StudentActivityRegistrationResult> {
	const response = await apiClient.post<StudentActivityRegistrationResult>(
		registrationPath(academicTermId, groupId)
	);
	return requireApiData(response, 'ลงทะเบียนกิจกรรมไม่สำเร็จ');
}

export async function unenrollMyActivityRegistration(
	academicTermId: string,
	groupId: string
): Promise<StudentActivityRegistrationResult> {
	const response = await apiClient.delete<StudentActivityRegistrationResult>(
		registrationPath(academicTermId, groupId)
	);
	return requireApiData(response, 'ยกเลิกการลงทะเบียนกิจกรรมไม่สำเร็จ');
}
