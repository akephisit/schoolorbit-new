import type {
	CatalogDisplayState,
	GradeLevelOption
} from '$lib/api/academic-core';

export type CatalogChoice = Readonly<{
	value: string;
	label: string;
}>;

export const SUBJECT_TYPE_OPTIONS = [
	{ value: 'BASIC', label: 'รายวิชาพื้นฐาน' },
	{ value: 'ADDITIONAL', label: 'รายวิชาเพิ่มเติม' },
	{ value: 'ACTIVITY', label: 'กิจกรรมพัฒนาผู้เรียน' }
] as const satisfies readonly CatalogChoice[];

export const ACTIVITY_TYPE_OPTIONS = [
	{ value: 'guidance', label: 'แนะแนว' },
	{ value: 'scout', label: 'ลูกเสือ / เนตรนารี / ยุวกาชาด' },
	{ value: 'club', label: 'ชุมนุม' },
	{ value: 'social', label: 'กิจกรรมเพื่อสังคมและสาธารณประโยชน์' },
	{ value: 'other', label: 'กิจกรรมอื่น ๆ' }
] as const satisfies readonly CatalogChoice[];

export const SCHEDULING_MODE_OPTIONS = [
	{ value: 'synchronized', label: 'จัดพร้อมกัน' },
	{ value: 'independent', label: 'จัดแยกเวลาได้' }
] as const satisfies readonly CatalogChoice[];

export const CATALOG_DISPLAY_STATE_OPTIONS = [
	{ value: 'current', label: 'ใช้อยู่ปัจจุบัน' },
	{ value: 'upcoming', label: 'รอเริ่มใช้' },
	{ value: 'expired', label: 'สิ้นสุดแล้ว' },
	{ value: 'unpublished', label: 'ยังไม่เผยแพร่' }
] as const satisfies readonly CatalogChoice[];

const DISPLAY_STATE_CLASSES: Record<CatalogDisplayState, string> = {
	current: 'border-emerald-600/20 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
	upcoming: 'border-sky-600/20 bg-sky-500/10 text-sky-700 dark:text-sky-300',
	expired: 'border-border bg-muted text-muted-foreground',
	unpublished: 'border-amber-600/20 bg-amber-500/10 text-amber-700 dark:text-amber-300'
};

const VERSION_STATUS_LABELS = {
	draft: 'ฉบับร่าง',
	published: 'เผยแพร่แล้ว',
	archived: 'เก็บถาวร'
} as const;

export function optionLabel(
	options: readonly CatalogChoice[],
	value: string | null | undefined,
	fallback = 'ไม่ระบุ'
): string {
	if (!value) return fallback;
	return options.find((option) => option.value === value)?.label ?? value;
}

export function displayStateLabel(state: CatalogDisplayState): string {
	return optionLabel(CATALOG_DISPLAY_STATE_OPTIONS, state);
}

export function displayStateClass(state: CatalogDisplayState): string {
	return DISPLAY_STATE_CLASSES[state];
}

export function versionStatusLabel(status: keyof typeof VERSION_STATUS_LABELS): string {
	return VERSION_STATUS_LABELS[status];
}

export function gradeLevelLabel(option: GradeLevelOption): string {
	return option.short_name || option.name || option.code;
}

export function gradeLevelSummary(options: readonly GradeLevelOption[]): string {
	if (options.length === 0) return 'ทุกระดับชั้น';
	return options.map(gradeLevelLabel).join(', ');
}

export function formatThaiDate(value: string | null | undefined): string {
	if (!value) return 'ไม่กำหนด';
	const [year, month, day] = value.split('-').map(Number);
	if (!year || !month || !day) return value;
	const date = new Date(Date.UTC(year, month - 1, day));
	return new Intl.DateTimeFormat('th-TH', {
		day: 'numeric',
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	}).format(date);
}

export function formatEffectiveRange(from: string, until?: string | null): string {
	return until
		? `${formatThaiDate(from)} – ${formatThaiDate(until)}`
		: `ตั้งแต่ ${formatThaiDate(from)}`;
}

export function normalizeCatalogSearch(value: string): string {
	return value.normalize('NFKC').trim().toLocaleLowerCase('th-TH');
}

export function matchesCatalogSearch(search: string, ...values: Array<string | null | undefined>) {
	const query = normalizeCatalogSearch(search);
	if (!query) return true;
	return values.some((value) => value && normalizeCatalogSearch(value).includes(query));
}
