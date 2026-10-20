import type { BudgetStatus } from '#lib/api/budget.js';
export const statusLabels: Record<BudgetStatus, string> = {
	draft: 'ฉบับร่าง',
	submitted: 'รอพิจารณา',
	returned: 'ส่งกลับแก้ไข',
	approved: 'อนุมัติแล้ว',
	rejected: 'ไม่อนุมัติ',
	closed: 'ปิดแล้ว'
};
export const adjustmentLabels = {
	add: 'เพิ่มงบประมาณ',
	reduce: 'ลดงบประมาณ',
	transfer: 'โอนงบประมาณ',
	loan: 'ยืมงบประมาณ',
	repayment: 'คืนเงินยืม'
} as const;
export function dateLabel(value: string): string {
	const date = new Date(`${value.slice(0, 10)}T00:00:00Z`);
	return Number.isFinite(date.getTime())
		? date.toLocaleDateString('th-TH', {
				timeZone: 'UTC',
				day: 'numeric',
				month: 'short',
				year: 'numeric'
			})
		: value;
}
export const pageSize = 20;
export function budgetFilters(url: URL) {
	const offset = Number(url.searchParams.get('offset') ?? '0');
	const status = url.searchParams.get('status') ?? '';
	return {
		fiscalYearId: url.searchParams.get('fiscalYearId') ?? '',
		projectId: url.searchParams.get('projectId') ?? '',
		activityId: url.searchParams.get('activityId') ?? '',
		search: url.searchParams.get('search') ?? '',
		status: status in statusLabels ? (status as BudgetStatus) : '',
		limit: pageSize,
		offset: Number.isSafeInteger(offset) && offset >= 0 ? offset : 0
	};
}
export function destination(
	path: string,
	values: Record<string, string | number | undefined>
): string {
	const query = new URLSearchParams();
	for (const [key, value] of Object.entries(values))
		if (value !== undefined && value !== '' && value !== 0) query.set(key, String(value));
	return `${path}${query.size ? `?${query}` : ''}`;
}
export function safeReturn(raw: string | null, fallback: string): string {
	if (!raw || !raw.startsWith('/staff/budget') || raw.startsWith('//')) return fallback;
	try {
		const parsed = new URL(raw, 'https://budget.invalid');
		return parsed.origin === 'https://budget.invalid' &&
			parsed.pathname.startsWith('/staff/budget/')
			? `${parsed.pathname}${parsed.search}`
			: fallback;
	} catch {
		return fallback;
	}
}
