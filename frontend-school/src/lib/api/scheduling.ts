import { apiClient } from './client';
import type { UUID } from '$lib/types';

// ==================== Types ====================

export interface Period {
	id: UUID;
	name: string;
	start_time: string;
	end_time: string;
	type: 'TEACHING' | 'BREAK' | 'ACTIVITY' | 'HOMEROOM';
	order_index: number;
	applicable_days?: string;
}

export const DAY_OPTIONS = [
	{ value: 'MON', label: 'จันทร์' },
	{ value: 'TUE', label: 'อังคาร' },
	{ value: 'WED', label: 'พุธ' },
	{ value: 'THU', label: 'พฤหัสบดี' },
	{ value: 'FRI', label: 'ศุกร์' },
	{ value: 'SAT', label: 'เสาร์' },
	{ value: 'SUN', label: 'อาทิตย์' }
] as const;

// Phase F: Timetable Templates
export interface TimetableTemplateView {
	id: UUID;
	name: string;
	description?: string;
	created_by?: UUID;
	created_at: string;
	updated_at: string;
	entry_count: number;
}

export interface TimetableTemplateEntry {
	id: UUID;
	template_id: UUID;
	day_of_week: string;
	period_id: UUID;
	entry_type: string;
	title?: string;
	activity_slot_id?: UUID;
	grade_level_ids: UUID[];
	classroom_ids: UUID[];
	instructor_ids: UUID[];
	room_id?: UUID;
}

export async function listTimetableTemplates() {
	return apiClient.get<TimetableTemplateView[]>('/api/academic/timetable-templates');
}

export async function getTimetableTemplate(id: UUID) {
	return apiClient.get<{
		template: TimetableTemplateView;
		entries: TimetableTemplateEntry[];
	}>(`/api/academic/timetable-templates/${id}`);
}

export async function updateTimetableTemplate(
	id: UUID,
	req: { name?: string; description?: string }
) {
	return apiClient.put<Record<string, never>>(`/api/academic/timetable-templates/${id}`, req);
}

export async function deleteTimetableTemplate(id: UUID) {
	return apiClient.delete<Record<string, never>>(`/api/academic/timetable-templates/${id}`);
}

export async function createTemplateFromCurrent(req: {
	semester_id: UUID;
	name: string;
	description?: string;
	entry_types?: string[];
}) {
	return apiClient.post<TimetableTemplateView>(
		'/api/academic/timetable-templates/from-current',
		req
	);
}

export async function applyTimetableTemplate(id: UUID, req: { semester_id: UUID }) {
	return apiClient.post<{ applied: number }>(`/api/academic/timetable-templates/${id}/apply`, req);
}

export async function clearTimetable(req: { semester_id: UUID; entry_types?: string[] }) {
	const response = await apiClient.deleteWithBody<{ deleted: number }>(
		'/api/academic/timetable/clear',
		req
	);
	if (!response.success) throw new Error(response.error || 'Clear timetable failed');
	return response;
}

export async function listPeriods(academicYearId?: UUID) {
	const params = academicYearId ? `?academic_year_id=${academicYearId}` : '';
	return apiClient.get<Period[]>(`/api/academic/periods${params}`);
}
