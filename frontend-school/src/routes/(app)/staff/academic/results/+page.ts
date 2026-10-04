import type { PageLoad } from './$types';
import {
	getAcademicResultReadiness,
	getActivityResultPreparation,
	getCourseResultPreparation,
	listAcademicGradingPolicies,
	type AcademicResultReadiness,
	type CourseResultPreparationWorkspace,
	type ActivityResultPreparationWorkspace
} from '#lib/api/academicResults.js';
import {
	getLearnerEvaluationWorkspace,
	getStudentLearnerEvaluationSummary,
	listLearnerEvaluationSubjects,
	type LearnerEvaluationSubject,
	type LearnerEvaluationWorkspace,
	type StudentLearnerEvaluationSummary
} from '#lib/api/academicLearnerEvaluations.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

type Section = 'course' | 'activity' | 'learner';
type Overview =
	| { section: 'result'; readiness: AcademicResultReadiness }
	| { section: 'learner'; subjects: LearnerEvaluationSubject[] };
type Summary = { studentId: string; data: StudentLearnerEvaluationSummary };
type Workspace =
	| { section: 'course'; groupId: string; data: CourseResultPreparationWorkspace }
	| { section: 'activity'; groupId: string; data: ActivityResultPreparationWorkspace }
	| {
			section: 'learner';
			groupId: string;
			desirable: LearnerEvaluationWorkspace;
			reading: LearnerEvaluationWorkspace;
	  };

function requestedSection(value: string | null): Section {
	return value === 'activity' || value === 'learner' ? value : 'course';
}

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'สรุปผลการเรียน',
		icon: 'ListChecks',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 30,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_RESULT_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_RESULT_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_RESULT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_SCHOOL,
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
	const section = requestedSection(url.searchParams.get('section'));
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	if (!context)
		return {
			title: _meta.menu.title,
			context,
			section,
			overview: null,
			policy: null,
			workspace: null,
			summary: null
		};

	const overview = captureRouteLoad<Overview>(
		section === 'learner'
			? listLearnerEvaluationSubjects(context, { requestFetch: fetch }).then((subjects) => ({
					section: 'learner' as const,
					subjects
				}))
			: getAcademicResultReadiness(context, { requestFetch: fetch }).then((readiness) => ({
					section: 'result' as const,
					readiness
				})),
		'โหลดข้อมูลเตรียมผลไม่สำเร็จ'
	);
	const policy =
		section === 'learner'
			? null
			: captureRouteLoad(
					listAcademicGradingPolicies(context, { requestFetch: fetch }),
					'โหลดเกณฑ์ตัดผลไม่สำเร็จ'
				);
	const loadSelectedGroup = (groupId: string): Promise<RouteLoadResult<Workspace>> => {
		if (section === 'course')
			return captureRouteLoad<Workspace>(
				getCourseResultPreparation(groupId, context, { requestFetch: fetch }).then((data) => ({
					section: 'course' as const,
					groupId,
					data
				})),
				'โหลดผลรายวิชาไม่สำเร็จ'
			);
		if (section === 'activity')
			return captureRouteLoad<Workspace>(
				getActivityResultPreparation(groupId, context, { requestFetch: fetch }).then((data) => ({
					section: 'activity' as const,
					groupId,
					data
				})),
				'โหลดผลกิจกรรมไม่สำเร็จ'
			);
		return captureRouteLoad<Workspace>(
			Promise.all([
				getLearnerEvaluationWorkspace(groupId, 'desirable_characteristic', context, {
					requestFetch: fetch
				}),
				getLearnerEvaluationWorkspace(groupId, 'reading_thinking_writing', context, {
					requestFetch: fetch
				})
			]).then(([desirable, reading]) => ({
				section: 'learner' as const,
				groupId,
				desirable,
				reading
			})),
			'โหลดผลประเมินผู้เรียนไม่สำเร็จ'
		);
	};
	const requestedGroupId = url.searchParams.get('learningGroupId')?.trim() || '';
	const workspace: Promise<RouteLoadResult<Workspace | null>> = requestedGroupId
		? loadSelectedGroup(requestedGroupId)
		: overview.then<RouteLoadResult<Workspace | null>>((result) => {
				if (!result.ok) return { ok: true as const, data: null, error: null };
				const groups =
					result.data.section === 'learner'
						? result.data.subjects
						: section === 'activity'
							? result.data.readiness.activities
							: result.data.readiness.courses.flatMap((course) => course.groups);
				const group = groups[0];
				return group
					? loadSelectedGroup(group.learningGroupId)
					: { ok: true as const, data: null, error: null };
			});
	const summary =
		section === 'learner'
			? workspace.then<RouteLoadResult<Summary | null>>((result) => {
					if (!result.ok || !result.data || result.data.section !== 'learner')
						return { ok: true as const, data: null, error: null };
					const requestedStudentId = url.searchParams.get('studentAcademicYearId')?.trim() || '';
					const student =
						result.data.desirable.students.find(
							(row) => row.studentAcademicYearId === requestedStudentId
						) ?? result.data.desirable.students[0];
					return student
						? captureRouteLoad(
								getStudentLearnerEvaluationSummary(student.studentAcademicYearId, context, {
									requestFetch: fetch
								}).then((data) => ({ studentId: student.studentAcademicYearId, data })),
								'โหลดผลสรุปผู้เรียนไม่สำเร็จ'
							)
						: { ok: true as const, data: null, error: null };
				})
			: null;
	return { title: _meta.menu.title, context, section, overview, policy, workspace, summary };
};
