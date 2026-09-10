import type { AcademicYear } from '$lib/api/academic-core';

export function canPlanTermsInYear(status: AcademicYear['status']): boolean {
	return status === 'planning' || status === 'active';
}

export function termAnnualFlags(included: boolean, blocks: boolean) {
	return { includedInYearResult: included, blocksYearClosure: included || blocks };
}
