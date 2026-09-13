import type { components } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];

export function aggregateStatus(
	row: Pick<Schemas['TermClosureStudent'], 'revisionId' | 'isCurrent'>
): 'missing' | 'stale' | 'current' {
	return !row.revisionId ? 'missing' : row.isCurrent ? 'current' : 'stale';
}

export function canLockAggregate(
	preview: Pick<
		Schemas['TermAggregatePreview'],
		'canLock' | 'blockers' | 'holdFindings' | 'policy'
	> | null,
	reason: string,
	allowed: boolean,
	stale: boolean
): boolean {
	if (!allowed || stale || !preview?.canLock || preview.blockers.length > 0) return false;
	if (preview.holdFindings.length === 0) return reason.trim() === '';
	return (
		preview.policy.allowReviewedHolds &&
		reason.trim().length > 0 &&
		Array.from(reason).length <= 1000
	);
}

export const aggregateStatusLabels = {
	missing: 'ยังไม่ล็อกผลสรุป',
	stale: 'ต้นทางเปลี่ยน ต้องตรวจใหม่',
	current: 'ผลสรุปล่าสุด'
};
export const aggregateBlockerLabels: Record<Schemas['AggregateBlocker'], string> = {
	no_course_coverage: 'ยังไม่มีรายวิชาสำหรับสรุปผล',
	missing_course_results: 'ยังขาดผลรายวิชาที่ล็อกแล้ว',
	missing_activity_results: 'ยังขาดผลกิจกรรมที่ล็อกแล้ว',
	missing_learner_evaluations: 'ยังขาดผลคุณลักษณะฯ หรือการอ่าน คิดวิเคราะห์ และเขียน',
	reviewed_hold_not_allowed: 'นโยบายนี้ยังไม่อนุญาตให้ล็อกพร้อมผลค้าง'
};
export const aggregateHoldLabels: Record<Schemas['AggregateHoldFinding'], string> = {
	exceptional_course_outcomes: 'มีผลรายวิชาที่ไม่ใช่เกรดตัวเลข',
	failed_activities: 'มีกิจกรรมที่ยังไม่ผ่าน',
	failed_learner_evaluations: 'มีผลประเมินต่ำกว่าเกณฑ์'
};
