import type {
	BudgetActivityInput,
	BudgetProjectInput,
	BudgetProjectWorkspace,
	BudgetPlanLineInput
} from '#lib/api/budget.js';
import type { SupplyItem } from '#lib/api/supply-catalog.js';
import { bahtInput, lineTotal, quantityMillis, toSatang } from './money.js';

export type LineDraft = Omit<BudgetPlanLineInput, 'unitPriceSatang'> & {
	key: string;
	price: string;
};
export type ActivityDraft = Omit<BudgetActivityInput, 'lines'> & {
	key: string;
	lines: LineDraft[];
};
export const newProject = (): BudgetProjectInput => ({
	name: '',
	fiscalYearId: '',
	organizationUnitId: '',
	startDate: '',
	endDate: ''
});
export const newActivity = (): ActivityDraft => ({
	key: crypto.randomUUID(),
	title: '',
	defaultFundId: null,
	defaultExpenseCategoryId: null,
	lines: []
});
export function newLine(
	activity: Pick<ActivityDraft, 'defaultFundId' | 'defaultExpenseCategoryId'>
): LineDraft {
	return {
		key: crypto.randomUUID(),
		description: '',
		quantity: '1',
		unit: '',
		price: '',
		fundId: activity.defaultFundId ?? '',
		expenseCategoryId: activity.defaultExpenseCategoryId ?? '',
		supplyItemId: null,
		supplyVersionId: null
	};
}
export function catalogLine(activity: ActivityDraft, item: SupplyItem): LineDraft {
	return {
		...newLine(activity),
		description: item.name,
		unit: item.unit,
		price: bahtInput(toSatang(item.latestVersion.price)),
		supplyItemId: item.id,
		supplyVersionId: item.latestVersion.id
	};
}
export function projectInput(workspace: BudgetProjectWorkspace): BudgetProjectInput {
	const { name, fiscalYearId, organizationUnitId, startDate, endDate } = workspace.project;
	return { name, fiscalYearId, organizationUnitId, startDate, endDate };
}
export function activityDrafts(workspace: BudgetProjectWorkspace): ActivityDraft[] {
	return workspace.activities.map((activity) => ({
		id: activity.id,
		key: activity.id,
		title: activity.title,
		defaultFundId: activity.defaultFundId,
		defaultExpenseCategoryId: activity.defaultExpenseCategoryId,
		lines: activity.lines.map((line) => ({
			id: line.id,
			key: line.id,
			description: line.description,
			quantity: line.quantity,
			unit: line.unit,
			price: bahtInput(line.unitPriceSatang),
			fundId: line.fundId,
			expenseCategoryId: line.expenseCategoryId,
			supplyItemId: line.supplyItemId,
			supplyVersionId: line.supplyVersionId
		}))
	}));
}
export function activityInput(activity: ActivityDraft): BudgetActivityInput {
	return {
		id: activity.id,
		title: activity.title.trim(),
		defaultFundId: activity.defaultFundId || null,
		defaultExpenseCategoryId: activity.defaultExpenseCategoryId || null,
		lines: activity.lines.map((line) => ({
			id: line.id,
			description: line.description.trim(),
			quantity: line.quantity,
			unit: line.unit.trim(),
			unitPriceSatang: toSatang(line.price),
			fundId: line.fundId,
			expenseCategoryId: line.expenseCategoryId,
			supplyItemId: line.supplyItemId || null,
			supplyVersionId: line.supplyVersionId || null
		}))
	};
}
export function applyDefaults(activity: ActivityDraft, selected: readonly string[]): ActivityDraft {
	const keys = new Set(selected);
	return {
		...activity,
		lines: activity.lines.map((line) =>
			keys.has(line.key)
				? {
						...line,
						fundId: activity.defaultFundId || line.fundId,
						expenseCategoryId: activity.defaultExpenseCategoryId || line.expenseCategoryId
					}
				: line
		)
	};
}
export function validDate(value: string): boolean {
	if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
	const date = new Date(`${value}T00:00:00Z`);
	return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === value;
}
export function projectErrors(
	draft: BudgetProjectInput
): Partial<Record<keyof BudgetProjectInput, string>> {
	const errors: Partial<Record<keyof BudgetProjectInput, string>> = {};
	if (!draft.name.trim()) errors.name = 'กรอกชื่อโครงการ';
	if (!draft.fiscalYearId) errors.fiscalYearId = 'เลือกปีงบประมาณ';
	if (!draft.organizationUnitId) errors.organizationUnitId = 'เลือกหน่วยงานที่รับผิดชอบ';
	if (!validDate(draft.startDate)) errors.startDate = 'เลือกวันที่เริ่มต้น';
	if (!validDate(draft.endDate) || draft.endDate < draft.startDate)
		errors.endDate = 'วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น';
	return errors;
}
export function lineErrors(
	line: LineDraft
): Partial<
	Record<'description' | 'quantity' | 'unit' | 'price' | 'fundId' | 'expenseCategoryId', string>
> {
	const errors: ReturnType<typeof lineErrors> = {};
	if (!line.description.trim()) errors.description = 'กรอกชื่อรายการ';
	if (!line.unit.trim()) errors.unit = 'กรอกหน่วยนับ';
	if (!line.fundId) errors.fundId = 'เลือกแหล่งเงิน';
	if (!line.expenseCategoryId) errors.expenseCategoryId = 'เลือกหมวดรายจ่าย';
	try {
		quantityMillis(line.quantity);
	} catch (cause) {
		errors.quantity = message(cause);
	}
	try {
		toSatang(line.price);
	} catch (cause) {
		errors.price = message(cause);
	}
	return errors;
}
export function previewTotal(line: LineDraft): string {
	try {
		return lineTotal(line.quantity, toSatang(line.price));
	} catch {
		return '0';
	}
}
export function message(cause: unknown): string {
	return cause instanceof Error ? cause.message : 'ดำเนินการไม่สำเร็จ กรุณาลองอีกครั้ง';
}
