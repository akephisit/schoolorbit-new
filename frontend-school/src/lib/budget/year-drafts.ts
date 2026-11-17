import type { BudgetYearSettings, SaveBudgetYearSettingsRequest } from '#lib/api/budget.js';
import { bahtInput, toSatang } from './money.js';

export type YearDraft = Omit<
	SaveBudgetYearSettingsRequest,
	'requestId' | 'authorizedFunding' | 'unitAllocations'
> & {
	funding: { fundId: string; carryover: string; annual: string; reserve: string }[] | null;
	allocations: { organizationUnitId: string; fundId: string; amount: string }[];
	received: Record<string, string>;
};
export function yearDraft(value: BudgetYearSettings): YearDraft {
	return {
		expectedRevision: value.fiscalYear.revision,
		name: value.fiscalYear.name,
		startDate: value.fiscalYear.startDate,
		endDate: value.fiscalYear.endDate,
		funds: structuredClone(value.funds),
		expenseCategories: structuredClone(value.expenseCategories),
		planSteps: structuredClone(value.planSteps),
		requestSteps: structuredClone(value.requestSteps),
		adjustmentSteps: structuredClone(value.adjustmentSteps),
		funding:
			value.authorizedFunding?.map((row) => ({
				fundId: row.fundId,
				carryover: bahtInput(row.carryoverSatang),
				annual: bahtInput(row.annualFundingSatang),
				reserve: bahtInput(row.reserveSatang)
			})) ?? null,
		allocations: value.unitAllocations.map((row) => ({
			organizationUnitId: row.organizationUnitId,
			fundId: row.fundId,
			amount: bahtInput(row.allocatedSatang)
		})),
		received: Object.fromEntries(
			value.sourceBalances.map((row) => [
				row.fundId,
				row.receivedSatang == null ? '' : bahtInput(row.receivedSatang)
			])
		)
	};
}
export function yearInput(draft: YearDraft): Omit<SaveBudgetYearSettingsRequest, 'requestId'> {
	const { funding, allocations, received: _received, ...settings } = draft;
	void _received;
	return {
		...settings,
		authorizedFunding:
			funding?.map((row) => ({
				fundId: row.fundId,
				carryoverSatang: toSatang(row.carryover),
				annualFundingSatang: toSatang(row.annual),
				reserveSatang: toSatang(row.reserve)
			})) ?? null,
		unitAllocations: allocations.map((row) => ({
			organizationUnitId: row.organizationUnitId,
			fundId: row.fundId,
			allocatedSatang: toSatang(row.amount)
		}))
	};
}
