import type {
	CorrespondenceNumberingSettings,
	CorrespondenceStampContext,
	CorrespondenceSummary,
	CorrespondenceTextStampTemplate
} from '#lib/api/correspondence.js';
import { correspondenceRegisterLabel } from './presentation.js';

export const stampTokens = [
	{ value: 'schoolName', label: 'ชื่อโรงเรียน' },
	{ value: 'registerNumber', label: 'เลขทะเบียน' },
	{ value: 'registeredDate', label: 'วันที่ลงทะเบียน' },
	{ value: 'registeredTime', label: 'เวลาลงทะเบียน' },
	{ value: 'date', label: 'วันที่ปัจจุบัน' },
	{ value: 'time', label: 'เวลาปัจจุบัน' },
	{ value: 'signerName', label: 'ชื่อผู้ดำเนินการ' },
	{ value: 'signerTitle', label: 'ตำแหน่ง' },
	{ value: 'instruction', label: 'ข้อความสั่งการ' }
] as const;

function tokens(body: string, allowed: readonly string[]): string[] | null {
	const found: string[] = [];
	let rest = body;
	while (/[{}]/u.test(rest)) {
		const start = rest.search(/[{}]/u);
		if (rest[start] !== '{') return null;
		const end = rest.indexOf('}', start + 1);
		if (end < 0) return null;
		const value = rest.slice(start + 1, end);
		if (!allowed.includes(value)) return null;
		found.push(value);
		rest = rest.slice(end + 1);
	}
	return found;
}

export function numberingError(value: CorrespondenceNumberingSettings): string | undefined {
	const found = tokens(value.format, ['number', 'year']);
	if (
		[...value.format].length > 100 ||
		/\p{Cc}/u.test(value.format) ||
		!Number.isInteger(value.minimumDigits) ||
		value.minimumDigits < 1 ||
		value.minimumDigits > 8
	)
		return 'รูปแบบไม่เกิน 100 ตัวอักษร และจำนวนหลักต้องเป็น 1–8';
	if (
		!found ||
		found.filter((token) => token === 'number').length !== 1 ||
		found.filter((token) => token === 'year').length > 1
	)
		return 'ใส่ {number} หนึ่งครั้ง และ {year} ได้ไม่เกินหนึ่งครั้ง';
	return undefined;
}

/** Example only; actual registration numbers and historical labels remain server-owned. */
export function numberingExample(value: CorrespondenceNumberingSettings): string {
	if (numberingError(value)) return '—';
	return value.format
		.replace('{number}', '1'.padStart(value.minimumDigits, '0'))
		.replace('{year}', '2569');
}

export function stampTemplateErrors(
	template: Pick<CorrespondenceTextStampTemplate, 'name' | 'body'>
) {
	const errors: { name?: string; body?: string } = {};
	if (!template.name.trim() || [...template.name].length > 100 || /\p{Cc}/u.test(template.name))
		errors.name = 'ระบุชื่อแม่แบบไม่เกิน 100 ตัวอักษร';
	if (
		!template.body.trim() ||
		[...template.body].length > 5000 ||
		/[^\P{Cc}\n\t]/u.test(template.body)
	)
		errors.body = 'ระบุข้อความไม่เกิน 5,000 ตัวอักษร';
	else if (
		!tokens(
			template.body,
			stampTokens.map((token) => token.value)
		)
	)
		errors.body = 'ใช้ได้เฉพาะตัวแปรที่แสดงด้านล่าง และปิดวงเล็บให้ครบ';
	return errors;
}

export function expandStamp(
	body: string,
	context: CorrespondenceStampContext,
	record: CorrespondenceSummary,
	input: { instruction: string; signerTitle: string },
	now = new Date()
): { text: string; errors: { instruction?: string; signerTitle?: string; body?: string } } {
	const found = tokens(
		body,
		stampTokens.map((token) => token.value)
	);
	const errors: { instruction?: string; signerTitle?: string; body?: string } = {};
	if (!found) return { text: '', errors: { body: 'แม่แบบมีตัวแปรที่ไม่รองรับ' } };
	if (found.includes('instruction') && !input.instruction.trim())
		errors.instruction = 'ระบุข้อความสั่งการ';
	if (found.includes('signerTitle') && !input.signerTitle.trim())
		errors.signerTitle = 'ระบุตำแหน่งสำหรับข้อความนี้';
	if (found.includes('schoolName') && !context.schoolName?.trim())
		errors.body = 'ยังไม่มีชื่อโรงเรียนสำหรับแม่แบบนี้ กรุณาแก้ไขข้อความก่อนวาง';
	const date = (value: Date) =>
		value.toLocaleDateString('th-TH', {
			timeZone: 'Asia/Bangkok',
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
	const time = (value: Date) =>
		value.toLocaleTimeString('th-TH', {
			timeZone: 'Asia/Bangkok',
			hour: '2-digit',
			minute: '2-digit',
			hour12: false
		});
	const registered = new Date(record.createdAt);
	const values: Record<string, string> = {
		schoolName: context.schoolName ?? '',
		registerNumber: correspondenceRegisterLabel(record),
		date: date(now),
		time: time(now),
		registeredDate: date(registered),
		registeredTime: time(registered),
		signerName: context.actorDisplayName,
		signerTitle: input.signerTitle.trim(),
		instruction: input.instruction.trim()
	};
	return { text: body.replace(/\{([^{}]+)\}/gu, (_, token: string) => values[token]), errors };
}
