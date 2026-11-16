import type {
	LeaveCoverageLesson,
	LeaveCoveragePlan,
	LeaveCoverageSelection,
	LeaveCoverageSettings,
	DatedTeachingLesson
} from '#lib/api/staff-leave.js';

export function coverageCounts(
	lessons: LeaveCoverageLesson[],
	selections?: LeaveCoverageSelection[]
) {
	const selected = selections ? new Map(selections.map((value) => [value.lessonKey, value])) : null;
	return {
		total: lessons.length,
		selected: lessons.filter((lesson) =>
			selected ? selected.has(lesson.lessonKey) : !!lesson.assignment
		).length,
		confirmed: lessons.filter((lesson) => {
			if (lesson.assignment?.consent !== 'confirmed') return false;
			if (!selected) return true;
			const value = selected.get(lesson.lessonKey),
				saved = lesson.assignment;
			return (
				value &&
				value.replacementTeacherId === saved.replacementTeacherId &&
				value.mode === saved.mode &&
				value.taughtOfferingId === saved.taughtOfferingId &&
				value.taughtActivity === saved.taughtActivity &&
				value.returnLessonKey === saved.returnLessonKey
			);
		}).length
	};
}
export function coverageSelections(plan: LeaveCoveragePlan): LeaveCoverageSelection[] {
	return plan.days.flatMap((day) =>
		day.lessons.flatMap((lesson) =>
			lesson.assignment
				? [
						{
							lessonKey: lesson.lessonKey,
							replacementTeacherId: lesson.assignment.replacementTeacherId,
							mode: lesson.assignment.mode,
							taughtOfferingId: lesson.assignment.taughtOfferingId ?? null,
							taughtActivity: lesson.assignment.taughtActivity ?? null,
							returnLessonKey: lesson.assignment.returnLessonKey ?? null
						}
					]
				: []
		)
	);
}
export function coverageReady(plan: LeaveCoveragePlan): boolean {
	const counts = coverageCounts(plan.days.flatMap((day) => day.lessons));
	return plan.timetableState === 'ready' && counts.selected === counts.total;
}
export function validDefaultContext(settings: LeaveCoverageSettings) {
	return (
		settings.contexts.find(
			(context) =>
				context.academicYearId === settings.defaultAcademicYearId &&
				context.academicTermId === settings.defaultAcademicTermId
		) ?? null
	);
}
export const coverageStateLabels: Record<LeaveCoveragePlan['state'], string> = {
	unplanned: 'ยังไม่ได้จัดแผน',
	incomplete: 'ยังเลือกครูไม่ครบ',
	awaiting_confirmation: 'รอครูตอบรับ',
	confirmed: 'แผนพร้อมแล้ว',
	emergency: 'คำขอเร่งด่วน รอจัดแผนให้ครบ',
	legacy_unverified: 'แผนเดิมยังตรวจสอบไม่ได้'
};
export const coverageConsentLabels = {
	pending: 'รอตอบรับ',
	confirmed: 'ตอบรับแล้ว',
	declined: 'ปฏิเสธแล้ว'
};

export function coverageLessonLabels(
	lesson: Pick<
		DatedTeachingLesson,
		'offeringName' | 'learningGroupName' | 'homeroomNames' | 'roomName'
	>
) {
	const group = lesson.learningGroupName?.trim();
	const offering = lesson.offeringName?.trim();
	const title = group || offering || 'กิจกรรมตามตาราง';
	const details = [
		...(offering && group && !group.includes(offering) ? [offering] : []),
		...lesson.homeroomNames.filter((name) => !title.includes(name)),
		...(lesson.roomName ? [lesson.roomName] : [])
	];
	return { title, details: details.join(' · ') };
}
