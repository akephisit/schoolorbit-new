import type { components } from '#lib/api/generated/school-api.js';
export const outcomeLabels: Record<components['schemas']['PromotionDecisionOutcome'], string> = {
	promote: 'เลื่อนชั้น',
	repeat: 'ซ้ำชั้น',
	graduate: 'จบการศึกษา',
	transfer_out: 'ย้ายออก',
	hold: 'พักไว้',
	conditional: 'มีเงื่อนไข (ประวัติเดิม)'
};
export const runStatusLabels: Record<components['schemas']['PromotionRunStatus'], string> = {
	draft: 'รายการเดิมที่ยังไม่เสร็จ',
	calculated: 'รายการเดิมที่ยังไม่เสร็จ',
	reviewed: 'รายการเดิมที่ยังไม่เสร็จ',
	approved: 'รายการเดิมที่ยังไม่เสร็จ',
	executing: 'กำลังดำเนินการ',
	completed: 'บันทึกครบแล้ว',
	failed: 'มีรายการติดขัด'
};
