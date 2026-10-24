import type {
	ProcessAction,
	ProcessTemplateStep,
	SignatureInput
} from '#lib/api/correspondence.js';
import { hasInk, type InkPath } from './geometry.ts';

export function actionRequirements(action: ProcessAction, step?: ProcessTemplateStep) {
	const completion = action === step?.kind;
	return {
		acceptsEvidence: !['read', 'reassign', 'close'].includes(action),
		comment:
			['reject', 'return_for_revision', 'report', 'reassign'].includes(action) ||
			(completion && !!step?.requiresComment),
		evidence: completion && !!step?.requiresEvidence,
		signature: action === 'sign' || (completion && !!step?.requiresSignature)
	};
}

export function validateSigningDraft(
	input: SignatureInput | undefined,
	paths: InkPath[],
	method: string,
	assetId: string,
	confirmed: boolean,
	documentRevision: number,
	hasLocalImage = false
): string | null {
	if (!input || !confirmed || (method === 'drawn' ? !hasInk(paths) : !assetId && !hasLocalImage))
		return 'ระบุลายมือชื่อและยืนยันการลงนาม';
	if (input.documentRevision !== documentRevision)
		return 'เอกสารมีฉบับใหม่ กรุณาตรวจสอบแล้วเลือกใช้ข้อมูลล่าสุดก่อนลงนาม';
	const { x, y, width, height, page } = input;
	if (
		!Number.isInteger(page) ||
		page < 1 ||
		page > 1000 ||
		![x, y, width, height].every(Number.isFinite) ||
		x < 0 ||
		y < 0 ||
		width <= 0 ||
		height <= 0 ||
		x + width > 1.000001 ||
		y + height > 1.000001
	)
		return 'ตำแหน่งและขนาดลายมือชื่อต้องอยู่ภายในหน้าเอกสาร';
	return null;
}

export function historyActionLabel(action: string): string {
	return (
		(
			{
				create: 'ลงทะเบียนหนังสือ',
				revise: 'บันทึกฉบับแก้ไข',
				send: 'ส่งหนังสือ',
				read: 'เปิดอ่าน',
				route: 'พิจารณาสั่งการ',
				acknowledge: 'รับทราบ',
				review: 'เสนอความเห็น',
				approve: 'อนุมัติ',
				execute: 'ดำเนินการแล้ว',
				sign: 'ลงนาม',
				reject: 'ไม่อนุมัติ',
				return_for_revision: 'ส่งกลับแก้ไข',
				report: 'ส่งผลการดำเนินงาน',
				close: 'ปิดเรื่อง',
				reassign: 'ส่งต่อผู้รับผิดชอบ'
			} as Record<string, string>
		)[action] ?? 'บันทึกการดำเนินการ'
	);
}
