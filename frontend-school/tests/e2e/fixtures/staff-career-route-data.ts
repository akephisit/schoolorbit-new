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
export function rankMilestone(
	overrides: Partial<Schemas['RankMilestone']> = {}
): Schemas['RankMilestone'] {
	return {
		asOf: '2026-10-03',
		status: 'incomplete',
		reasons: ['missing_position_date'],
		reasonLabels: ['ยังไม่ระบุวันที่มีผลของตำแหน่งครู'],
		nextRank: 'senior_proficient',
		recordedStartDate: null,
		ordinaryDate: null,
		conditionalReducedDate: null,
		daysUntilOrdinaryDate: null,
		criteria: {
			version: 'teacher-ordinary-2026-10-03',
			effectiveFrom: '2026-10-03',
			reviewedOn: '2026-10-03',
			ordinaryYears: 4,
			conditionalReducedYears: 3,
			sources: [
				{
					title: '1932/2567',
					url: 'https://otepc.go.th/th/content_page/item/5169-2024-11-22-11-47-11.html'
				}
			]
		},
		...overrides
	};
}

export function rankOverview(
	overrides: Partial<Schemas['RankMilestoneOverview']> = {}
): Schemas['RankMilestoneOverview'] {
	return {
		asOf: '2026-10-03',
		bucket: 'due_soon',
		counts: {
			dueSoon: 0,
			future: 0,
			timeReachedPendingReview: 0,
			incomplete: 2,
			unsupported: 0,
			noNextRank: 0
		},
		filteredTotal: 2,
		items: [],
		total: 0,
		page: 1,
		pageSize: 50,
		...overrides
	};
}
