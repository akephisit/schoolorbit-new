import type { PageLoad } from './$types';
import {
	getGradebookGroupPhaseWorkspace,
	listGradebookSubjects,
	type GradebookPhaseCode,
	type GradebookSubject,
	type GroupPhaseWorkspace
} from '$lib/api/academicGradebook';
import {
	getLearnerEvaluationWorkspace,
	listLearnerEvaluationSubjects,
	type LearnerEvaluationDomain,
	type LearnerEvaluationSubject,
	type LearnerEvaluationWorkspace
} from '$lib/api/academicLearnerEvaluations';
import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

type GradebookTab = 'scores' | LearnerEvaluationDomain;
type SubjectRegion =
	| { tab: 'scores'; rows: GradebookSubject[] }
	| { tab: LearnerEvaluationDomain; rows: LearnerEvaluationSubject[] };
type WorkspaceRegion =
	| { tab: 'scores'; subjectId: string; groupId: string; rows: GroupPhaseWorkspace[] }
	| {
			tab: LearnerEvaluationDomain;
			subjectId: string;
			groupId: string;
			workspace: LearnerEvaluationWorkspace;
	  }
	| null;

const phaseCodes: GradebookPhaseCode[] = ['before_midterm', 'midterm', 'after_midterm', 'final'];

function requestedTab(value: string | null): GradebookTab {
	return value === 'desirable_characteristic' || value === 'reading_thinking_writing'
		? value
		: 'scores';
}

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'กรอกคะแนนและประเมินผู้เรียน',
		icon: 'BookOpenCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_GRADEBOOK_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_GRADEBOOK_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL
		]
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const tab = requestedTab(url.searchParams.get('tab'));
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	if (!context) return { title: _meta.menu.title, context, tab, subjects: null, workspace: null };

	const subjects: Promise<RouteLoadResult<SubjectRegion>> = captureRouteLoad(
		tab === 'scores'
			? listGradebookSubjects(context, { requestFetch: fetch }).then(
					(rows) => ({ tab, rows }) satisfies SubjectRegion
				)
			: listLearnerEvaluationSubjects(context, { requestFetch: fetch }).then(
					(rows) => ({ tab, rows }) satisfies SubjectRegion
				),
		'โหลดรายวิชาสำหรับกรอกข้อมูลไม่สำเร็จ'
	);
	const workspace: Promise<RouteLoadResult<WorkspaceRegion>> = subjects.then((result) => {
		if (!result.ok) return { ok: true, data: null, error: null };
		const requestedSubjectId = url.searchParams.get('subjectId')?.trim() || '';
		const requestedGroupId = url.searchParams.get('learningGroupId')?.trim() || '';
		const selected =
			result.data.rows.find(
				(row) => row.subjectId === requestedSubjectId && row.learningGroupId === requestedGroupId
			) ??
			result.data.rows.find((row) => row.subjectId === requestedSubjectId) ??
			result.data.rows[0];
		if (!selected) return { ok: true, data: null, error: null };
		const subjectId = selected.subjectId;
		const groupId = selected.learningGroupId;
		return captureRouteLoad(
			tab === 'scores'
				? Promise.all(
						phaseCodes
							.filter((code) =>
								(selected as GradebookSubject).phases.some((phase) => phase.phaseCode === code)
							)
							.map((code) =>
								getGradebookGroupPhaseWorkspace(groupId, code, context, { requestFetch: fetch })
							)
					).then((rows) => ({ tab, subjectId, groupId, rows }) satisfies WorkspaceRegion)
				: getLearnerEvaluationWorkspace(groupId, tab, context, { requestFetch: fetch }).then(
						(workspace) => ({ tab, subjectId, groupId, workspace }) satisfies WorkspaceRegion
					),
			'โหลดพื้นที่กรอกข้อมูลไม่สำเร็จ'
		);
	});
	return { title: _meta.menu.title, context, tab, subjects, workspace };
};
