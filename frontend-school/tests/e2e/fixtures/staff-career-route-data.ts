import type { components } from '../../../src/lib/api/generated/school-api';
import { firstStaff, id } from './staff-home-route-data';
type Schemas = components['schemas'];
export function careerEntry(
	overrides: Partial<Schemas['StaffCareerEntry']> = {}
): Schemas['StaffCareerEntry'] {
	return {
		id: id(91),
		staffId: firstStaff,
		fact: { kind: 'academic_rank', value: 'none' },
		jobPosition: null,
		effectiveDate: null,
		orderDate: null,
		orderNumber: null,
		note: null,
		source: 'existing_record',
		revision: 1,
		isCurrent: true,
		createdAt: '2026-10-01T00:00:00Z',
		updatedAt: '2026-10-01T00:00:00Z',
		...overrides
	};
}
export function personnelInfo(
	overrides: Partial<Schemas['StaffInfoResponse']> = {}
): Schemas['StaffInfoResponse'] {
	return {
		job_position: null,
		academic_rank: null,
		current_career: { personnelType: null, jobPosition: null, academicRank: null },
		education_level: null,
		major: null,
		university: null,
		...overrides
	};
}
