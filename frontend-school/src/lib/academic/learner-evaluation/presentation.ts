export interface LearnerEvaluationDomainSummaryLike {
	average?: { decimal: string; numerator?: string; denominator?: string } | null;
	catalogCriteria?: unknown[];
	complete: boolean;
	domain?: string;
	missingSubjects: Array<{ reason: string; subjectId?: string }>;
	qualityLevel?: number | null;
	subjects?: unknown[];
}

export function learnerEvaluationQualityLabel(level: number | null | undefined): string {
	switch (level) {
		case 3:
			return 'ดีเยี่ยม';
		case 2:
			return 'ดี';
		case 1:
			return 'ผ่าน';
		case 0:
			return 'ไม่ผ่าน';
		default:
			return 'ยังสรุปไม่ได้';
	}
}

export function presentLearnerEvaluationDomain(
	domain: LearnerEvaluationDomainSummaryLike,
	subjectLabels: Readonly<Record<string, string>> = {}
) {
	return {
		status: domain.complete ? ('complete' as const) : ('provisional' as const),
		statusLabel: domain.complete ? 'สรุปครบแล้ว' : 'ผลชั่วคราว',
		averageLabel: domain.average?.decimal ?? '—',
		qualityLabel: learnerEvaluationQualityLabel(domain.qualityLevel),
		reasons: domain.complete
			? []
			: domain.missingSubjects.map((subject) => {
					const label = subject.subjectId ? subjectLabels[subject.subjectId] : undefined;
					const reason = missingLearnerSubjectReasonLabel(subject.reason);
					return label ? `${label} · ${reason}` : reason;
				})
	};
}

export function missingLearnerSubjectReasonLabel(reason: string): string {
	switch (reason) {
		case 'subject_domain_not_locked':
			return 'ยังไม่ได้ล็อกผลด้านนี้';
		case 'student_not_in_locked_snapshot':
			return 'นักเรียนไม่อยู่ในข้อมูลที่ล็อกไว้';
		default:
			return reason;
	}
}

export function learnerEvaluationLockBlockerLabel(reason: string): string {
	switch (reason) {
		case 'no_active_criteria':
			return 'รายวิชายังไม่มีหัวข้อประเมินที่ใช้งาน';
		case 'missing_responses':
			return 'ยังประเมินนักเรียนไม่ครบทุกหัวข้อ';
		case 'group_not_confirmed':
			return 'ครูหลักยังไม่ยืนยันผลของกลุ่มเรียน';
		case 'stale_group_confirmation':
			return 'ข้อมูลเปลี่ยนหลังยืนยัน กรุณายืนยันกลุ่มเรียนใหม่';
		case 'student_in_multiple_subject_groups':
			return 'นักเรียนอยู่ซ้ำมากกว่าหนึ่งกลุ่มในรายวิชานี้';
		default:
			return reason;
	}
}
