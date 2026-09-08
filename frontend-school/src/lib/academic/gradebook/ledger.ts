export interface GradebookCellPosition {
	itemId: string;
	studentId: string;
}

export type GradebookCellDirection = 'tab_forward' | 'tab_backward' | 'enter_down' | 'enter_up';

export interface ScorePasteMutation extends GradebookCellPosition {
	value: string | null;
}

export type ScorePasteErrorCode =
	| 'no_selected_columns'
	| 'origin_not_editable'
	| 'origin_not_found'
	| 'non_rectangular'
	| 'invalid_decimal'
	| 'out_of_bounds'
	| 'too_many_cells';

export interface ScorePasteError {
	code: ScorePasteErrorCode;
	message: string;
	row?: number;
	column?: number;
	value?: string;
}

export type ScorePasteResult =
	| { ok: true; mutations: ScorePasteMutation[] }
	| { ok: false; error: ScorePasteError };

const MAX_PASTE_CELLS = 500;
const SCORE_DECIMAL = /^(0|[1-9]\d*)(\.\d{1,2})?$/;

export function formatGradebookScore(value: string | null | undefined): string | null {
	if (value == null) return null;
	if (!value.includes('.')) return value;
	return value.replace(/0+$/, '').replace(/\.$/, '');
}

function uniqueIds(ids: readonly string[]): string[] {
	return [...new Set(ids.filter((id) => id.length > 0))];
}

export function clearGradebookItemSelection(): string[] {
	return [];
}

export function selectAllGradebookItems(itemIds: readonly string[]): string[] {
	return uniqueIds(itemIds);
}

export function selectNewGradebookItem(
	selectedItemIds: readonly string[],
	itemId: string
): string[] {
	return uniqueIds([...selectedItemIds, itemId]);
}

export function toggleGradebookItemSelection(
	selectedItemIds: readonly string[],
	itemId: string
): string[] {
	const selected = uniqueIds(selectedItemIds);
	return selected.includes(itemId)
		? selected.filter((selectedId) => selectedId !== itemId)
		: [...selected, itemId];
}

export function nextEditableCell(
	position: GradebookCellPosition,
	selectedItemIds: readonly string[],
	studentIds: readonly string[],
	direction: GradebookCellDirection
): GradebookCellPosition | null {
	const items = uniqueIds(selectedItemIds);
	const students = uniqueIds(studentIds);
	if (items.length === 0 || students.length === 0) return null;

	const studentIndex = students.indexOf(position.studentId);
	if (studentIndex < 0) return null;
	const itemIndex = items.indexOf(position.itemId);
	if (itemIndex < 0) {
		return {
			itemId: direction === 'tab_backward' || direction === 'enter_up' ? items.at(-1)! : items[0]!,
			studentId: position.studentId
		};
	}

	const total = items.length * students.length;
	if (direction === 'tab_forward' || direction === 'tab_backward') {
		const current = studentIndex * items.length + itemIndex;
		const delta = direction === 'tab_forward' ? 1 : -1;
		const next = (current + delta + total) % total;
		return {
			itemId: items[next % items.length]!,
			studentId: students[Math.floor(next / items.length)]!
		};
	}

	const current = itemIndex * students.length + studentIndex;
	const delta = direction === 'enter_down' ? 1 : -1;
	const next = (current + delta + total) % total;
	return {
		itemId: items[Math.floor(next / students.length)]!,
		studentId: students[next % students.length]!
	};
}

function pasteError(code: ScorePasteErrorCode, message: string): ScorePasteResult {
	return { ok: false, error: { code, message } };
}

function clipboardRows(text: string): string[][] {
	const lines = text.replace(/\r\n?/g, '\n').split('\n');
	while (lines.length > 1 && lines.at(-1) === '') lines.pop();
	return lines.map((line) => line.split('\t'));
}

export function normalizeScorePaste(
	text: string,
	origin: GradebookCellPosition,
	selectedItemIds: readonly string[],
	studentIds: readonly string[]
): ScorePasteResult {
	const items = uniqueIds(selectedItemIds);
	const students = uniqueIds(studentIds);
	if (items.length === 0) {
		return pasteError('no_selected_columns', 'กรุณาเลือกคอลัมน์คะแนนที่ต้องการกรอกก่อน');
	}

	const originColumn = items.indexOf(origin.itemId);
	if (originColumn < 0) {
		return pasteError('origin_not_editable', 'คอลัมน์ต้นทางยังไม่ได้เลือกให้แก้ไข');
	}
	const originRow = students.indexOf(origin.studentId);
	if (originRow < 0) {
		return pasteError('origin_not_found', 'ไม่พบนักเรียนของช่องต้นทางในกลุ่มเรียนปัจจุบัน');
	}

	const rows = clipboardRows(text);
	const width = rows[0]?.length ?? 0;
	if (rows.some((row) => row.length !== width)) {
		return pasteError('non_rectangular', 'ข้อมูลที่วางต้องมีจำนวนคอลัมน์เท่ากันทุกแถว');
	}
	if (rows.length * width > MAX_PASTE_CELLS) {
		return pasteError(
			'too_many_cells',
			`วางคะแนนได้ไม่เกิน ${MAX_PASTE_CELLS.toLocaleString('th-TH')} ช่องต่อครั้ง`
		);
	}
	if (originColumn + width > items.length || originRow + rows.length > students.length) {
		return pasteError('out_of_bounds', 'ข้อมูลที่วางเกินขอบเขตคอลัมน์หรือนักเรียนที่เลือก');
	}

	const mutations: ScorePasteMutation[] = [];
	for (const [rowOffset, row] of rows.entries()) {
		for (const [columnOffset, rawValue] of row.entries()) {
			const value = rawValue.trim();
			if (value !== '' && !SCORE_DECIMAL.test(value)) {
				return {
					ok: false,
					error: {
						code: 'invalid_decimal',
						message: `คะแนนแถว ${rowOffset + 1} คอลัมน์ ${columnOffset + 1} ไม่ใช่ตัวเลขที่รองรับ`,
						row: rowOffset,
						column: columnOffset,
						value: rawValue
					}
				};
			}
			mutations.push({
				itemId: items[originColumn + columnOffset]!,
				studentId: students[originRow + rowOffset]!,
				value: value === '' ? null : value
			});
		}
	}

	return { ok: true, mutations };
}
