import type { components } from '#lib/api/generated/school-api.js';

export type LifecycleFinding = components['schemas']['LifecycleFinding'];

const groups = [
	{ key: 'period', label: 'สถานะปีและภาคเรียน' },
	{ key: 'time', label: 'ปฏิทินและตารางเวลา' },
	{ key: 'roster', label: 'นักเรียนและห้องเรียน' },
	{ key: 'learning', label: 'ข้อมูลการเรียนและเงื่อนไขอื่น' }
] as const;

function groupKey(code: string): (typeof groups)[number]['key'] {
	if (/calendar|bell_schedule|timetable|term_configuration|year_overlap/.test(code)) return 'time';
	if (/student|placement|room_capacity|enrollment|roster/.test(code)) return 'roster';
	if (
		/year_state|term_state|running_year|running_term|earlier_year|earlier_term|first_term|source_state|target_state/.test(
			code
		)
	)
		return 'period';
	return 'learning';
}

export function groupReadinessFindings(findings: LifecycleFinding[]) {
	return groups.flatMap((group) => {
		const items = findings.filter((finding) => groupKey(finding.code) === group.key);
		return items.length ? [{ ...group, findings: items }] : [];
	});
}
