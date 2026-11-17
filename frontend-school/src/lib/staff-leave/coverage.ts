import type {
	LeaveCoverageLesson,
	LeaveCoverageAssignment,
	LeaveCoveragePlan,
	LeaveCoverageSelection,
	LeaveCoverageOptions,
	DatedTeachingLesson
} from '#lib/api/staff-leave.js';

export function coverageSelectionMatches(
	lesson: LeaveCoverageLesson,
	selection: LeaveCoverageSelection | undefined
): boolean {
	const saved = lesson.assignment;
	return (
		!!saved &&
		!!selection &&
		selection.lessonKey === lesson.lessonKey &&
		selection.replacementTeacherId === saved.replacementTeacherId &&
		selection.mode === saved.mode &&
		(selection.taughtOfferingId ?? null) === (saved.taughtOfferingId ?? null) &&
		(selection.taughtActivity ?? null) === (saved.taughtActivity ?? null) &&
		(selection.returnLessonKey ?? null) === (saved.returnLessonKey ?? null)
	);
}

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
			return coverageSelectionMatches(lesson, selected.get(lesson.lessonKey));
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
export function validDefaultContext(settings: LeaveCoverageOptions) {
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

export function coverageReturnCaption(
	lesson: Pick<LeaveCoverageLesson, 'offeringName'>,
	assignment: Pick<LeaveCoverageAssignment, 'returnLesson'>,
	dateLabel: string
): string {
	const slot = assignment.returnLesson;
	const parts = [
		dateLabel,
		slot?.periodName,
		slot ? `${slot.startTime.slice(0, 5)}–${slot.endTime.slice(0, 5)}` : null
	].filter((value): value is string => !!value?.trim());
	const displaced = slot?.offeringName?.trim();
	return `${lesson.offeringName?.trim() || 'กิจกรรมตามตาราง'} คืน ${parts.join(' · ')}${displaced ? ` (แทนคาบ${displaced})` : ''}`;
}
