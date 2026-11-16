import type {
	SupervisionCycleStatus,
	SupervisionObservation,
	SupervisionObservationStatus
} from '#lib/api/supervision.js';

const statusLabels = {
	draft: 'ร่าง',
	open: 'เปิดรับคำขอ',
	closed: 'ปิดรับคำขอ',
	archived: 'เก็บถาวร',
	requested: 'รออนุมัติคำขอ',
	returned: 'ส่งกลับให้แก้ไข',
	planned: 'นัดหมายแล้ว',
	in_progress: 'กำลังประเมิน',
	evaluators_submitted: 'รอรับรองผล',
	under_review: 'รอรับรองผล',
	approved: 'รออนุมัติผล',
	published: 'รอครูรับทราบ',
	acknowledged: 'เสร็จสิ้น',
	completed: 'เสร็จสิ้น',
	cancelled: 'ยกเลิก'
} satisfies Record<SupervisionCycleStatus | SupervisionObservationStatus, string>;

export function supervisionStatusLabel(
	status: SupervisionCycleStatus | SupervisionObservationStatus
): string {
	return statusLabels[status];
}

export const supervisionStages = [
	'คำขอ',
	'ประเมิน',
	'รับรองผล',
	'อนุมัติผล',
	'รับทราบ',
	'เสร็จสิ้น'
];

const workflowByStatus = {
	requested: { stage: 0, nextOwner: 'ผู้รับผิดชอบตรวจคำขอ', nextAction: 'ตรวจและอนุมัติคำขอ' },
	returned: { stage: 0, nextOwner: 'ครูผู้ขอรับนิเทศ', nextAction: 'แก้ไขและส่งใหม่' },
	planned: { stage: 1, nextOwner: 'ผู้ประเมิน', nextAction: 'นิเทศและส่งผลประเมิน' },
	in_progress: { stage: 1, nextOwner: 'ผู้ประเมิน', nextAction: 'ส่งผลประเมินที่ยังค้าง' },
	evaluators_submitted: {
		stage: 2,
		nextOwner: 'ผู้มีสิทธิ์รับรองผล',
		nextAction: 'ตรวจและรับรองผล'
	},
	under_review: { stage: 2, nextOwner: 'ผู้มีสิทธิ์รับรองผล', nextAction: 'ตรวจและรับรองผล' },
	approved: { stage: 3, nextOwner: 'ผู้มีสิทธิ์อนุมัติผล', nextAction: 'ตรวจและอนุมัติผล' },
	published: { stage: 4, nextOwner: 'ครูผู้รับการนิเทศ', nextAction: 'อ่านและรับทราบผล' },
	acknowledged: { stage: 5, nextOwner: 'ไม่มีงานค้าง', nextAction: 'รับทราบผลแล้ว' },
	completed: { stage: 5, nextOwner: 'ไม่มีงานค้าง', nextAction: 'รับทราบผลแล้ว' },
	cancelled: { stage: -1, nextOwner: 'ไม่มีงานค้าง', nextAction: 'รายการถูกยกเลิก' }
} satisfies Record<
	SupervisionObservationStatus,
	{ stage: number; nextOwner: string; nextAction: string }
>;

// Presentation follows the server status; evaluator counts never advance the workflow locally.
export function observationWorkflow(
	observation: Pick<SupervisionObservation, 'status' | 'evaluators'>
) {
	const workflow = workflowByStatus[observation.status];
	const required = observation.evaluators.filter((evaluator) => evaluator.isRequired);
	const pendingRequired = required.filter((evaluator) => evaluator.status !== 'submitted');
	const pendingNames = pendingRequired.map(
		(evaluator) => evaluator.evaluatorDisplayName ?? 'ผู้ประเมิน'
	);
	return {
		...workflow,
		stageLabel:
			workflow.stage < 0
				? 'ยุติรายการ'
				: `ขั้น ${workflow.stage + 1}/${supervisionStages.length} ${supervisionStages[workflow.stage]}`,
		nextOwner:
			workflow.stage === 1 && pendingNames.length ? pendingNames.join(', ') : workflow.nextOwner,
		evaluatorProgress: `ผู้ประเมินที่จำเป็นส่งแล้ว ${required.length - pendingRequired.length}/${required.length} คน`,
		pendingRequiredLabel: pendingNames.length
			? `ยังรอผู้ประเมินที่จำเป็น: ${pendingNames.join(', ')}`
			: null
	};
}

export function currentReturnReason(
	observation: Pick<SupervisionObservation, 'status' | 'actions'>
): string | null {
	if (observation.status !== 'returned') return null;
	const action = observation.actions
		.filter((item) => item.toStatus === 'returned')
		.sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
	return action?.comment?.trim() || null;
}
