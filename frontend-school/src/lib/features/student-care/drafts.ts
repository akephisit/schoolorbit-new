import { z } from 'zod';
import type {
	CareInstrumentDefinition,
	CareAssignmentDetail,
	CareAnswer,
	CareManualScore,
	SaveCareResponse
} from '#lib/api/student-care.js';
const key = z
	.string({ error: 'กรอกรหัสอ้างอิงเป็นข้อความ' })
	.regex(/^[a-zA-Z0-9_-]{1,60}$/, 'ใช้ตัวอักษรอังกฤษ ตัวเลข _ หรือ - ไม่เกิน 60 ตัว');
const band = z.strictObject({
	key,
	label: z
		.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
		.trim()
		.min(1, 'กรอกข้อมูลนี้ให้ครบ')
		.max(200, 'ใช้ไม่เกิน 200 ตัวอักษร'),
	minimum: z.int({ error: 'ระบุจำนวนเต็ม' }).min(-200000, 'ค่าต้องไม่น้อยกว่า -200,000'),
	maximum: z.int({ error: 'ระบุจำนวนเต็ม' }).max(200000, 'ค่าต้องไม่เกิน 200,000'),
	urgent: z.boolean()
});
const option = z.strictObject({
	key,
	label: z
		.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
		.trim()
		.min(1, 'กรอกข้อมูลนี้ให้ครบ')
		.max(500, 'ใช้ไม่เกิน 500 ตัวอักษร'),
	points: z
		.int({ error: 'ระบุจำนวนเต็ม' })
		.min(-1000, 'ค่าต้องไม่น้อยกว่า -1,000')
		.max(1000, 'ค่าต้องไม่เกิน 1,000'),
	urgent: z.boolean()
});
export const careDefinitionSchema = z
	.strictObject({
		name: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.trim()
			.min(1, 'กรอกข้อมูลนี้ให้ครบ')
			.max(200, 'ใช้ไม่เกิน 200 ตัวอักษร'),
		family: z.enum(['sdq', 'eq', 'depression', 'other']),
		variant: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.trim()
			.min(1, 'กรอกข้อมูลนี้ให้ครบ')
			.max(120, 'ใช้ไม่เกิน 120 ตัวอักษร'),
		language: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.trim()
			.min(1, 'กรอกข้อมูลนี้ให้ครบ')
			.max(30, 'ใช้ไม่เกิน 30 ตัวอักษร'),
		respondent: z.enum(['self_report', 'teacher', 'guardian']),
		minimumAge: z.int({ error: 'ระบุจำนวนเต็ม' }).min(0, 'ค่าต้องไม่น้อยกว่า 0'),
		maximumAge: z.int({ error: 'ระบุจำนวนเต็ม' }).max(120, 'อายุต้องไม่เกิน 120 ปี'),
		mode: z.enum(['digital', 'manual_result']),
		instructions: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.max(10000, 'ใช้ไม่เกิน 10,000 ตัวอักษร'),
		source: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.trim()
			.min(1, 'กรอกข้อมูลนี้ให้ครบ')
			.max(2000, 'ใช้ไม่เกิน 2,000 ตัวอักษร'),
		licenseNote: z
			.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
			.trim()
			.min(1, 'กรอกข้อมูลนี้ให้ครบ')
			.max(2000, 'ใช้ไม่เกิน 2,000 ตัวอักษร'),
		digitalRightsConfirmed: z.boolean(),
		questions: z
			.array(
				z.strictObject({
					key,
					text: z
						.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
						.trim()
						.min(1, 'กรอกข้อมูลนี้ให้ครบ')
						.max(2000, 'ใช้ไม่เกิน 2,000 ตัวอักษร'),
					scaleKey: key,
					reverse: z.boolean(),
					options: z
						.array(option)
						.min(2, 'เพิ่มตัวเลือกอย่างน้อย 2 ตัวเลือก')
						.max(20, 'เพิ่มได้ไม่เกิน 20 ตัวเลือก')
				})
			)
			.max(200, 'เพิ่มคำถามได้ไม่เกิน 200 ข้อ'),
		scales: z
			.array(
				z.strictObject({
					key,
					label: z
						.string({ error: 'กรอกข้อมูลเป็นข้อความ' })
						.trim()
						.min(1, 'กรอกข้อมูลนี้ให้ครบ')
						.max(200, 'ใช้ไม่เกิน 200 ตัวอักษร'),
					sourceScaleKeys: z.array(key),
					bands: z
						.array(band)
						.min(1, 'เพิ่มช่วงคะแนนอย่างน้อย 1 ช่วง')
						.max(30, 'เพิ่มช่วงคะแนนได้ไม่เกิน 30 ช่วง')
				})
			)
			.min(1, 'เพิ่มมาตรวัดอย่างน้อย 1 มาตรวัด')
			.max(30, 'เพิ่มมาตรวัดได้ไม่เกิน 30 มาตรวัด')
	})
	.superRefine((d, ctx) => {
		const issue = (path: (string | number)[], message: string) =>
			ctx.addIssue({ code: 'custom', path, message });
		if (d.maximumAge < d.minimumAge) issue(['maximumAge'], 'อายุสูงสุดต้องไม่น้อยกว่าอายุต่ำสุด');
		if (d.mode === 'digital' && !d.digitalRightsConfirmed)
			issue(['digitalRightsConfirmed'], 'ยืนยันสิทธิ์เผยแพร่คำถามทางดิจิทัลก่อนบันทึก');
		if (d.mode === 'digital' && !d.questions.length)
			issue(['questions'], 'เพิ่มคำถามก่อนใช้แบบออนไลน์');
		if (d.mode === 'manual_result' && d.questions.length)
			issue(['questions'], 'แบบบันทึกคะแนนกระดาษต้องไม่มีคำถามออนไลน์');
		const unique = (values: string[]) => new Set(values).size === values.length;
		if (!unique(d.scales.map((s) => s.key))) issue(['scales'], 'รหัสมาตรวัดซ้ำ');
		if (!unique(d.questions.map((q) => q.key))) issue(['questions'], 'รหัสคำถามซ้ำ');
		d.scales.forEach((s, i) => {
			if (
				!unique(s.sourceScaleKeys) ||
				s.sourceScaleKeys.some(
					(k) =>
						k === s.key || !d.scales.some((base) => base.key === k && !base.sourceScaleKeys.length)
				)
			)
				issue(
					['scales', i, 'sourceScaleKeys'],
					'มาตรวัดรวมต้องอ้างอิงมาตรวัดพื้นฐานที่มีอยู่โดยไม่ซ้ำหรือวนรอบ'
				);
			const bands = [...s.bands].sort((a, b) => a.minimum - b.minimum);
			if (
				!unique(bands.map((b) => b.key)) ||
				bands.some(
					(b, j) => b.minimum > b.maximum || (j > 0 && bands[j - 1].maximum + 1 !== b.minimum)
				)
			)
				issue(
					['scales', i, 'bands'],
					'ช่วงคะแนนต้องเรียงต่อเนื่อง ไม่ซ้ำ และค่าต่ำสุดไม่เกินค่าสูงสุด'
				);
			if (d.mode === 'digital' && !s.sourceScaleKeys.length) {
				const questions = d.questions.filter((q) => q.scaleKey === s.key);
				const min = questions.reduce(
					(sum, q) => sum + Math.min(...q.options.map((o) => o.points)),
					0
				);
				const max = questions.reduce(
					(sum, q) => sum + Math.max(...q.options.map((o) => o.points)),
					0
				);
				if (
					!questions.length ||
					!bands.some((b) => b.minimum <= min && b.maximum >= min) ||
					!bands.some((b) => b.minimum <= max && b.maximum >= max)
				)
					issue(['scales', i, 'bands'], 'ช่วงคะแนนต้องครอบคลุมผลรวมคำตอบทั้งหมดของมาตรวัด');
			}
		});

		d.scales.forEach((scale, index) => {
			if (!scale.sourceScaleKeys.length) return;
			const bases = scale.sourceScaleKeys.map((key) => d.scales.find((base) => base.key === key));
			if (bases.some((base) => !base || base.sourceScaleKeys.length)) return;
			const minimum = bases.reduce(
				(sum, base) => sum + Math.min(...(base?.bands.map((band) => band.minimum) ?? [])),
				0
			);
			const maximum = bases.reduce(
				(sum, base) => sum + Math.max(...(base?.bands.map((band) => band.maximum) ?? [])),
				0
			);
			if (
				!scale.bands.some((band) => band.minimum <= minimum && band.maximum >= minimum) ||
				!scale.bands.some((band) => band.minimum <= maximum && band.maximum >= maximum)
			)
				issue(['scales', index, 'bands'], 'ช่วงคะแนนมาตรวัดรวมต้องครอบคลุมผลรวมของมาตรวัดพื้นฐาน');
		});
		d.questions.forEach((q, i) => {
			if (!d.scales.some((s) => s.key === q.scaleKey && !s.sourceScaleKeys.length))
				issue(['questions', i, 'scaleKey'], 'เลือกมาตรวัดพื้นฐานที่มีอยู่');
			if (!unique(q.options.map((o) => o.key)))
				issue(['questions', i, 'options'], 'รหัสตัวเลือกซ้ำ');
		});
	});
export function parseCareDefinition(json: string): CareInstrumentDefinition {
	return careDefinitionSchema.parse(JSON.parse(json));
}
export function emptyCareDefinition(): CareInstrumentDefinition {
	return {
		name: '',
		family: 'other',
		variant: '',
		language: 'th',
		respondent: 'self_report',
		minimumAge: 6,
		maximumAge: 18,
		mode: 'manual_result',
		instructions: '',
		source: '',
		licenseNote: '',
		digitalRightsConfirmed: false,
		questions: [],
		scales: []
	};
}
export function definitionErrors(definition: CareInstrumentDefinition): string[] {
	const result = careDefinitionSchema.safeParse(definition);
	return result.success
		? []
		: result.error.issues.map((issue) => `${issue.path.join('.')}: ${issue.message}`);
}
export function setCareAnswer(
	answers: CareAnswer[],
	questionKey: string,
	optionKey: string
): CareAnswer[] {
	return [
		...answers.filter((a) => a.questionKey !== questionKey),
		...(optionKey ? [{ questionKey, optionKey }] : [])
	];
}
export function responseError(
	detail: CareAssignmentDetail,
	answers: CareAnswer[],
	scores: CareManualScore[],
	complete: boolean
): string | null {
	const definition = detail.instrument.definition;
	if (definition.mode === 'digital') {
		if (
			answers.some(
				(a) =>
					!definition.questions.some(
						(q) => q.key === a.questionKey && q.options.some((o) => o.key === a.optionKey)
					)
			) ||
			new Set(answers.map((a) => a.questionKey)).size !== answers.length
		)
			return 'คำตอบไม่ตรงกับแบบประเมินรุ่นนี้';
		if (complete && answers.length !== definition.questions.length) return 'ตอบให้ครบทุกข้อก่อนส่ง';
	} else {
		const base = definition.scales.filter((s) => !s.sourceScaleKeys.length);
		if (
			scores.some(
				(s) =>
					!base.some((b) => b.key === s.scaleKey) ||
					!Number.isSafeInteger(s.score) ||
					!base
						.find((b) => b.key === s.scaleKey)
						?.bands.some((b) => b.minimum <= s.score && b.maximum >= s.score)
			) ||
			new Set(scores.map((s) => s.scaleKey)).size !== scores.length
		)
			return 'ตรวจสอบคะแนนแต่ละมาตรวัดและช่วงคะแนนที่กำหนด';
		if (
			complete &&
			(scores.length !== base.length ||
				scores.some(
					(s) =>
						!base
							.find((b) => b.key === s.scaleKey)
							?.bands.some((b) => b.minimum <= s.score && b.maximum >= s.score)
				))
		)
			return 'กรอกคะแนนทุกมาตรวัดให้ตรงช่วงคะแนนที่กำหนด';
	}
	return null;
}
export function careResponsePayload(
	detail: CareAssignmentDetail,
	answers: CareAnswer[],
	manualScores: CareManualScore[],
	manualNote: string,
	acknowledgePurpose: boolean,
	requestId: string
): SaveCareResponse {
	return {
		requestId,
		rowVersion: detail.assignment.rowVersion,
		answers: detail.instrument.definition.mode === 'digital' ? answers : [],
		manualScores: detail.instrument.definition.mode === 'manual_result' ? manualScores : [],
		manualNote: manualNote.trim() || null,
		acknowledgePurpose
	};
}

export function careImportError(error: unknown): string {
	if (error instanceof z.ZodError)
		return error.issues
			.map((issue) =>
				issue.code === 'unrecognized_keys'
					? `มีข้อมูลที่ไม่รองรับ: ${issue.keys.join(', ')}`
					: `${issue.path.join('.') || 'ข้อมูลแบบประเมิน'}: ${issue.message}`
			)
			.join('\n');
	return 'ตรวจสอบรูปแบบ JSON ให้ถูกต้องก่อนนำเข้า';
}
