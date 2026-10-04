<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import { aggregateCapabilities } from '#lib/academic/results/aggregate-access.js';
	import {
		sortAssignedFirst,
		type CourseOutcomeSelection
	} from '#lib/academic/results/presentation.js';

	import {
		confirmActivityGroupResults,
		confirmCourseGroupResults,
		getAcademicResultReadiness,
		getActivityResultPreparation,
		getCourseResultPreparation,
		listAcademicGradingPolicies,
		saveActivityResultOutcomes,
		saveCourseResultSelection,
		type AcademicGradingPolicyVersion,
		type AcademicResultReadiness,
		type ActivityResultBatchInput,
		type ActivityResultPreparationWorkspace,
		type CourseResultPreparationWorkspace
	} from '#lib/api/academicResults.js';
	import {
		getLearnerEvaluationWorkspace,
		getStudentLearnerEvaluationSummary,
		listLearnerEvaluationSubjects,
		type LearnerEvaluationDomain,
		type LearnerEvaluationSubject,
		type LearnerEvaluationWorkspace,
		type StudentLearnerEvaluationSummary
	} from '#lib/api/academicLearnerEvaluations.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import AcademicPrerequisiteNotice from '#lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import ActivityEvaluationTable from '#lib/components/academic/results/ActivityEvaluationTable.svelte';
	import LearnerEvaluationSummary from '#lib/components/academic/results/LearnerEvaluationSummary.svelte';
	import ResultPreparationTable from '#lib/components/academic/results/ResultPreparationTable.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { BookOpenCheck, CheckCircle2, ClipboardCheck, Shapes, ShieldCheck } from '@lucide/svelte';

	type ResultSection = 'course' | 'activity' | 'learner';
	type CourseGroupRow = AcademicResultReadiness['courses'][number]['groups'][number] & {
		subjectId: string;
		code: string;
		name: string;
	};

	let { data }: PageProps = $props();
	const overviewRequest = new LatestRequest();
	const policyRequest = new LatestRequest();
	const workspaceRequest = new LatestRequest();
	const summaryRequest = new LatestRequest();
	const emptyReadiness: AcademicResultReadiness = { courses: [], activities: [] };
	const domains: LearnerEvaluationDomain[] = [
		'desirable_characteristic',
		'reading_thinking_writing'
	];

	let readiness = $state.raw<AcademicResultReadiness>(emptyReadiness);
	let resultOverviewLoaded = $state(false);
	let learnerOverviewLoaded = $state(false);
	let policies = $state.raw<AcademicGradingPolicyVersion[]>([]);
	let evaluationSubjects = $state.raw<LearnerEvaluationSubject[]>([]);
	let courseWorkspace = $state.raw<CourseResultPreparationWorkspace | null>(null);
	let activityWorkspace = $state.raw<ActivityResultPreparationWorkspace | null>(null);
	let evaluationWorkspaces = $state.raw<
		Partial<Record<LearnerEvaluationDomain, LearnerEvaluationWorkspace>>
	>({});
	let learnerSummary = $state.raw<StudentLearnerEvaluationSummary | null>(null);
	let activeSection = $state<ResultSection>(sectionFromUrl());
	let selectedGroupId = $state(page.url.searchParams.get('learningGroupId')?.trim() ?? '');
	let selectedStudentId = $state(page.url.searchParams.get('studentAcademicYearId')?.trim() ?? '');
	let loading = $state(true);
	let readinessUpdating = $state(false);
	let readinessError = $state('');
	let policyLoading = $state(true);
	let policyError = $state('');
	let policyLoaded = $state(false);
	let workspaceLoading = $state(true);
	let summaryLoading = $state(true);
	let errorMessage = $state('');
	let workspaceError = $state('');
	let summaryError = $state('');
	let busyStudentId = $state('');
	let confirming = $state(false);

	const academicYearId = $derived(data.context?.academicYearId ?? null);
	const academicTermId = $derived(data.context?.academicTermId ?? null);
	const canReadAggregate = $derived(aggregateCapabilities($can).read);
	const canReadResult = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_RESULT_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_RESULT_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_RESULT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_SCHOOL
		)
	);
	const canReadLearnerEvaluation = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL
		)
	);
	const canManageResult = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_RESULT_MANAGE_SCHOOL
		)
	);
	const activePolicy = $derived(policies.find((policy) => policy.lifecycle === 'active') ?? null);
	const courseGroups = $derived.by(() => {
		const subjectByGroup = new Map(
			evaluationSubjects.map((subject) => [subject.learningGroupId, subject] as const)
		);
		const groups: CourseGroupRow[] = readiness.courses.flatMap((subject) =>
			subject.groups.map((group) => {
				const known = subjectByGroup.get(group.learningGroupId);
				return {
					...group,
					subjectId: subject.subjectId,
					code: known?.code ?? '',
					name: known?.name ?? group.offeringName
				};
			})
		);
		return sortAssignedFirst(groups);
	});
	const activityGroups = $derived(sortAssignedFirst(readiness.activities));
	const learnerGroups = $derived(sortAssignedFirst(evaluationSubjects));
	const currentCourseGroup = $derived(
		courseGroups.find((group) => group.learningGroupId === selectedGroupId) ?? null
	);
	const currentActivityGroup = $derived(
		activityGroups.find((group) => group.learningGroupId === selectedGroupId) ?? null
	);
	const currentLearnerGroup = $derived(
		learnerGroups.find((group) => group.learningGroupId === selectedGroupId) ?? null
	);
	const learnerStudents = $derived(
		evaluationWorkspaces.desirable_characteristic?.students ??
			evaluationWorkspaces.reading_thinking_writing?.students ??
			[]
	);
	const learnerSubjectLabels = $derived.by(() => {
		const labels: Record<string, string> = {};
		for (const subject of evaluationSubjects) {
			labels[subject.subjectId] = `${subject.code} · ${subject.name}`;
		}
		return labels;
	});

	function sectionFromUrl(): ResultSection {
		const section = page.url.searchParams.get('section');
		return section === 'activity' || section === 'learner' ? section : 'course';
	}

	function contextValue() {
		return academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	}

	function syncUrl(): void {
		const url = new URL(page.url.href);
		url.searchParams.set('section', activeSection);

		if (selectedGroupId) url.searchParams.set('learningGroupId', selectedGroupId);
		else url.searchParams.delete('learningGroupId');
		if (selectedStudentId) url.searchParams.set('studentAcademicYearId', selectedStudentId);
		else url.searchParams.delete('studentAcademicYearId');

		goto(resolve(`staff/academic/results?${url.searchParams.toString()}`), {
			shallow: true,
			replace: true,
			state: page.state
		});
	}

	async function loadOverview(): Promise<void> {
		const context = contextValue();
		if (!context) return;
		const section = activeSection;
		const { revision, signal } = overviewRequest.begin();
		readinessUpdating = false;
		readinessError = '';
		loading = true;
		errorMessage = '';
		try {
			const result =
				section === 'learner'
					? await listLearnerEvaluationSubjects(context, { signal })
					: await getAcademicResultReadiness(context, { signal });
			if (!overviewRequest.isCurrent(revision)) return;
			if (section === 'learner') {
				evaluationSubjects = result as LearnerEvaluationSubject[];
				learnerOverviewLoaded = true;
			} else {
				readiness = result as AcademicResultReadiness;
				resultOverviewLoaded = true;
			}
			await ensureSelection();
		} catch (error) {
			if (isAbortError(error)) return;
			if (overviewRequest.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดข้อมูลเตรียมผลไม่สำเร็จ';
				workspaceLoading = false;
			}
		} finally {
			if (overviewRequest.isCurrent(revision)) loading = false;
		}
	}

	async function loadPolicy(): Promise<void> {
		const context = contextValue();
		if (!context || !canReadResult) return;
		const { revision, signal } = policyRequest.begin();
		policyLoading = true;
		policyError = '';
		try {
			const result = await listAcademicGradingPolicies(context, { signal });
			if (policyRequest.isCurrent(revision)) {
				policies = result;
				policyLoaded = true;
			}
		} catch (error) {
			if (!isAbortError(error) && policyRequest.isCurrent(revision))
				policyError = error instanceof Error ? error.message : 'โหลดเกณฑ์ตัดผลไม่สำเร็จ';
		} finally {
			if (policyRequest.isCurrent(revision)) policyLoading = false;
		}
	}

	async function refreshReadiness(): Promise<void> {
		const context = contextValue();
		if (!context || !canReadResult) return;
		const { revision, signal } = overviewRequest.begin();
		readinessUpdating = true;
		readinessError = '';
		try {
			const result = await getAcademicResultReadiness(context, { signal });
			if (overviewRequest.isCurrent(revision)) readiness = result;
		} catch (error) {
			if (!isAbortError(error) && overviewRequest.isCurrent(revision))
				readinessError = error instanceof Error ? error.message : 'อัปเดตสถานะความพร้อมไม่สำเร็จ';
		} finally {
			if (overviewRequest.isCurrent(revision)) readinessUpdating = false;
		}
	}

	function availableGroups(): Array<{ learningGroupId: string }> {
		if (activeSection === 'course') return courseGroups;
		if (activeSection === 'activity') return activityGroups;
		return learnerGroups;
	}

	async function ensureSelection(): Promise<void> {
		if (!canReadResult && activeSection !== 'learner') activeSection = 'learner';
		if (!canReadLearnerEvaluation && activeSection === 'learner') activeSection = 'course';
		const groups = availableGroups();
		if (!groups.some((group) => group.learningGroupId === selectedGroupId)) {
			selectedGroupId = groups[0]?.learningGroupId ?? '';
			selectedStudentId = '';
		}
		syncUrl();
		await loadSelectedWorkspace();
	}

	async function changeSection(section: string): Promise<void> {
		if (section !== 'course' && section !== 'activity' && section !== 'learner') return;
		workspaceRequest.abort();
		summaryRequest.abort();
		activeSection = section;
		selectedGroupId = '';
		selectedStudentId = '';
		courseWorkspace = null;
		activityWorkspace = null;
		evaluationWorkspaces = {};
		learnerSummary = null;
		workspaceError = '';
		workspaceLoading = true;
		readinessError = '';
		syncUrl();
		if (section !== 'learner' && !policyLoaded) void loadPolicy();
		if (section === 'learner' ? !learnerOverviewLoaded : !resultOverviewLoaded)
			await loadOverview();
		else await ensureSelection();
	}

	async function changeGroup(groupId: string): Promise<void> {
		selectedGroupId = groupId;
		selectedStudentId = '';
		syncUrl();
		await loadSelectedWorkspace();
	}

	async function loadSelectedWorkspace(): Promise<void> {
		workspaceRequest.abort();
		summaryRequest.abort();
		courseWorkspace = null;
		activityWorkspace = null;
		evaluationWorkspaces = {};
		learnerSummary = null;
		workspaceError = '';
		summaryError = '';
		if (!selectedGroupId) {
			workspaceLoading = false;
			return;
		}
		const context = contextValue();
		if (!context) {
			workspaceLoading = false;
			return;
		}
		const { revision, signal } = workspaceRequest.begin();
		workspaceLoading = true;
		try {
			if (activeSection === 'course') {
				const result = await getCourseResultPreparation(selectedGroupId, context, { signal });
				if (!workspaceRequest.isCurrent(revision)) return;
				courseWorkspace = result;
			} else if (activeSection === 'activity') {
				const result = await getActivityResultPreparation(selectedGroupId, context, {
					signal
				});
				if (!workspaceRequest.isCurrent(revision)) return;
				activityWorkspace = result;
			} else {
				const [desirable, reading] = await Promise.all(
					domains.map((domain) =>
						getLearnerEvaluationWorkspace(selectedGroupId, domain, context, { signal })
					)
				);
				if (!workspaceRequest.isCurrent(revision)) return;
				evaluationWorkspaces = {
					desirable_characteristic: desirable,
					reading_thinking_writing: reading
				};
				if (
					!desirable.students.some((student) => student.studentAcademicYearId === selectedStudentId)
				) {
					selectedStudentId = desirable.students[0]?.studentAcademicYearId ?? '';
				}
				syncUrl();
				if (selectedStudentId) void loadLearnerSummary(selectedStudentId);
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (workspaceRequest.isCurrent(revision)) {
				workspaceError = error instanceof Error ? error.message : 'โหลดข้อมูลกลุ่มเรียนไม่สำเร็จ';
			}
		} finally {
			if (workspaceRequest.isCurrent(revision)) workspaceLoading = false;
		}
	}

	async function selectCourseOutcome(
		studentAcademicYearId: string,
		selection: CourseOutcomeSelection,
		rowVersion: number | null
	): Promise<void> {
		if (!canManageResult) return;
		const context = contextValue();
		if (!context || !courseWorkspace) return;
		busyStudentId = studentAcademicYearId;
		try {
			courseWorkspace = await saveCourseResultSelection(selectedGroupId, context, {
				studentAcademicYearId,
				selection,
				rowVersion
			});
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกผลที่เลือกไม่สำเร็จ');
		} finally {
			busyStudentId = '';
		}
	}

	async function selectActivityOutcome(
		studentAcademicYearId: string,
		outcome: NonNullable<ActivityResultBatchInput['cells'][number]['outcome']> | null,
		rowVersion: number | null
	): Promise<void> {
		if (!canManageResult) return;
		const context = contextValue();
		if (!context || !activityWorkspace) return;
		busyStudentId = studentAcademicYearId;
		try {
			activityWorkspace = await saveActivityResultOutcomes(selectedGroupId, context, {
				cells: [{ studentAcademicYearId, outcome, rowVersion }]
			});
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกผลกิจกรรมไม่สำเร็จ');
		} finally {
			busyStudentId = '';
		}
	}

	async function confirmCourse(): Promise<void> {
		if (!canManageResult) return;
		const context = contextValue();
		if (!context || !courseWorkspace) return;
		confirming = true;
		try {
			courseWorkspace = await confirmCourseGroupResults(selectedGroupId, context, {
				sourceChecksum: courseWorkspace.sourceChecksum,
				rosterChecksum: courseWorkspace.rosterChecksum,
				rowVersion: courseWorkspace.confirmation?.rowVersion ?? null
			});
			toast.success('ยืนยันผลห้องนี้แล้ว');
			void refreshReadiness();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ยืนยันผลห้องนี้ไม่สำเร็จ');
		} finally {
			confirming = false;
		}
	}

	async function confirmActivity(): Promise<void> {
		if (!canManageResult) return;
		const context = contextValue();
		if (!context || !activityWorkspace) return;
		confirming = true;
		try {
			activityWorkspace = await confirmActivityGroupResults(selectedGroupId, context, {
				sourceChecksum: activityWorkspace.sourceChecksum,
				rosterChecksum: activityWorkspace.rosterChecksum,
				rowVersion: activityWorkspace.confirmation?.rowVersion ?? null
			});
			toast.success('ยืนยันผลกิจกรรมแล้ว');
			void refreshReadiness();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ยืนยันผลกิจกรรมไม่สำเร็จ');
		} finally {
			confirming = false;
		}
	}

	async function changeStudent(studentId: string): Promise<void> {
		selectedStudentId = studentId;
		syncUrl();
		await loadLearnerSummary(studentId);
	}

	async function loadLearnerSummary(studentId: string): Promise<void> {
		if (!canReadLearnerEvaluation) return;
		const context = contextValue();
		if (!context || !studentId) return;
		const { revision, signal } = summaryRequest.begin();
		summaryLoading = true;
		summaryError = '';
		try {
			const result = await getStudentLearnerEvaluationSummary(studentId, context, { signal });
			if (summaryRequest.isCurrent(revision)) learnerSummary = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (summaryRequest.isCurrent(revision)) {
				summaryError = error instanceof Error ? error.message : 'โหลดผลประเมินสรุปไม่สำเร็จ';
			}
		} finally {
			if (summaryRequest.isCurrent(revision)) summaryLoading = false;
		}
	}

	onDestroy(() => {
		overviewRequest.abort();
		policyRequest.abort();
		workspaceRequest.abort();
		summaryRequest.abort();
	});

	$effect.pre(() => {
		const routeOverview = data.overview;
		const section = data.section;
		const { revision } = overviewRequest.begin();
		untrack(() => {
			activeSection = section;
			readiness = emptyReadiness;
			evaluationSubjects = [];
			resultOverviewLoaded = false;
			learnerOverviewLoaded = false;
			readinessUpdating = false;
			readinessError = '';
			loading = Boolean(routeOverview);
			errorMessage = '';
		});
		if (routeOverview) {
			void routeOverview.then((result) => {
				if (!overviewRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						if (result.data.section === 'learner') {
							evaluationSubjects = result.data.subjects;
							learnerOverviewLoaded = true;
						} else {
							readiness = result.data.readiness;
							resultOverviewLoaded = true;
						}
						const groups =
							result.data.section === 'learner'
								? result.data.subjects
								: section === 'activity'
									? result.data.readiness.activities
									: result.data.readiness.courses.flatMap((course) => course.groups);
						const requestedGroupId = page.url.searchParams.get('learningGroupId')?.trim() ?? '';
						selectedGroupId =
							groups.find((group) => group.learningGroupId === requestedGroupId)?.learningGroupId ??
							groups[0]?.learningGroupId ??
							'';
						syncUrl();
					} else errorMessage = result.error;
					loading = false;
				});
			});
		}
		return () => {
			if (overviewRequest.isCurrent(revision)) overviewRequest.abort();
		};
	});

	$effect.pre(() => {
		const routePolicy = data.policy;
		const { revision } = policyRequest.begin();
		untrack(() => {
			policies = [];
			policyLoaded = false;
			policyLoading = Boolean(routePolicy);
			policyError = '';
		});
		if (routePolicy) {
			void routePolicy.then((result) => {
				if (!policyRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						policies = result.data;
						policyLoaded = true;
					} else policyError = result.error;
					policyLoading = false;
				});
			});
		}
		return () => {
			if (policyRequest.isCurrent(revision)) policyRequest.abort();
		};
	});

	$effect.pre(() => {
		const routeWorkspace = data.workspace;
		const { revision } = workspaceRequest.begin();
		untrack(() => {
			courseWorkspace = null;
			activityWorkspace = null;
			evaluationWorkspaces = {};
			selectedGroupId = '';
			workspaceLoading = Boolean(routeWorkspace);
			workspaceError = '';
		});
		if (routeWorkspace) {
			void routeWorkspace.then((result) => {
				if (!workspaceRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok && result.data) {
						selectedGroupId = result.data.groupId;
						if (result.data.section === 'course') courseWorkspace = result.data.data;
						else if (result.data.section === 'activity') activityWorkspace = result.data.data;
						else {
							evaluationWorkspaces = {
								desirable_characteristic: result.data.desirable,
								reading_thinking_writing: result.data.reading
							};
						}
						syncUrl();
					} else if (!result.ok) workspaceError = result.error;
					workspaceLoading = false;
				});
			});
		}
		return () => {
			if (workspaceRequest.isCurrent(revision)) workspaceRequest.abort();
		};
	});

	$effect.pre(() => {
		const routeSummary = data.summary;
		const { revision } = summaryRequest.begin();
		untrack(() => {
			learnerSummary = null;
			selectedStudentId = '';
			summaryLoading = Boolean(routeSummary);
			summaryError = '';
		});
		if (routeSummary) {
			void routeSummary.then((result) => {
				if (!summaryRequest.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok && result.data) {
						selectedStudentId = result.data.studentId;
						learnerSummary = result.data.data;
						syncUrl();
					} else if (!result.ok) summaryError = result.error;
					summaryLoading = false;
				});
			});
		}
		return () => {
			if (summaryRequest.isCurrent(revision)) summaryRequest.abort();
		};
	});

	$effect(() => {
		if (!academicTermId) return;
		if (!canReadResult && canReadLearnerEvaluation && activeSection !== 'learner')
			void changeSection('learner');
		else if (canReadResult && !canReadLearnerEvaluation && activeSection === 'learner')
			void changeSection('course');
	});
</script>

<PageShell
	title="สรุปผลการเรียน"
	description="ตรวจผลที่ระบบคำนวณ ยืนยันรายห้อง ประเมินกิจกรรม และดูผลประเมินผู้เรียนประจำภาคเรียน"
>
	{#snippet actions()}
		{#if canReadAggregate && academicYearId}
			<Button
				variant="outline"
				href={`/staff/academic/results/annual?academicYearId=${academicYearId}`}>สรุปผลรายปี</Button
			>
		{/if}
		{#if canReadAggregate && academicYearId && academicTermId}
			<Button
				variant="outline"
				href={`staff/academic/results/aggregates?academicYearId=${academicYearId}&academicTermId=${academicTermId}`}
				>สรุปผลรายภาค</Button
			>
		{/if}
	{/snippet}
	{#if !canReadResult && !canReadLearnerEvaluation}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูผลการเรียน"
			description="ติดต่อผู้ดูแลเพื่อขอสิทธิ์ตามรายวิชาหรือขอบเขตงานวิชาการ"
		/>
	{:else if !academicYearId || !academicTermId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="ใช้ตัวเลือกบนแถบด้านบนเพื่อเปิดผลการเรียนของภาคเรียนที่ต้องการ"
		/>
	{:else}
		<div class="space-y-4">
			<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]">
				<Card.Root class="gap-3 py-4" aria-busy={readinessUpdating}>
					<Card.Header class="px-4">
						<Card.Title class="text-base">เลือกงานที่ต้องการเตรียม</Card.Title>
						<Card.Description>วิชาและกิจกรรมที่บัญชีนี้รับผิดชอบจะแสดงก่อน</Card.Description>
						{#if readinessUpdating}<RegionUpdatingState
								class="static"
								label="กำลังอัปเดตความพร้อม..."
							/>{/if}
						{#if readinessError}<div
								role="alert"
								class="flex items-center gap-2 text-sm text-destructive"
							>
								<span>{readinessError}</span><Button
									variant="outline"
									size="sm"
									onclick={() => void refreshReadiness()}>ลองใหม่</Button
								>
							</div>{/if}
					</Card.Header>
					<Card.Content class="px-4">
						{#if loading}
							<PageSkeleton variant="form" rows={2} />
						{:else if errorMessage}
							<PageState
								variant="error"
								title="โหลดรายการกลุ่มไม่สำเร็จ"
								description={errorMessage}
								actionLabel="ลองอีกครั้ง"
								onaction={() => void loadOverview()}
							/>
						{:else}
							<Tabs.Root value={activeSection} onValueChange={(value) => void changeSection(value)}>
								<Tabs.List class="grid h-auto w-full grid-cols-1 gap-1 p-1 sm:grid-cols-3">
									<Tabs.Trigger value="course" disabled={!canReadResult}
										><ClipboardCheck class="size-4" /> รายวิชา</Tabs.Trigger
									>
									<Tabs.Trigger value="activity" disabled={!canReadResult}
										><Shapes class="size-4" /> กิจกรรม</Tabs.Trigger
									>
									<Tabs.Trigger value="learner" disabled={!canReadLearnerEvaluation}
										><ShieldCheck class="size-4" /> ผลประเมินผู้เรียน</Tabs.Trigger
									>
								</Tabs.List>
							</Tabs.Root>
							<div class="mt-4 space-y-1.5">
								<Label for="result-group"
									>{activeSection === 'activity'
										? 'กิจกรรมและกลุ่ม'
										: 'รายวิชาและกลุ่มเรียน'}</Label
								>
								<Select.Root
									type="single"
									value={selectedGroupId}
									onValueChange={(value) => void changeGroup(value)}
								>
									<Select.Trigger id="result-group" class="w-full">
										{#if activeSection === 'course'}
											{currentCourseGroup
												? `${currentCourseGroup.code ? `${currentCourseGroup.code} · ` : ''}${currentCourseGroup.name} · ${currentCourseGroup.groupName}`
												: 'เลือกกลุ่มเรียน'}
										{:else if activeSection === 'activity'}
											{currentActivityGroup
												? `${currentActivityGroup.offeringName} · ${currentActivityGroup.groupName}`
												: 'เลือกกลุ่มกิจกรรม'}
										{:else}
											{currentLearnerGroup
												? `${currentLearnerGroup.code} · ${currentLearnerGroup.name} · ${currentLearnerGroup.groupName}`
												: 'เลือกกลุ่มเรียน'}
										{/if}
									</Select.Trigger>
									<Select.Content>
										{#if activeSection === 'course'}
											{#each courseGroups as group (group.learningGroupId)}<Select.Item
													value={group.learningGroupId}
													>{group.code ? `${group.code} · ` : ''}{group.name} · {group.groupName}{group.assigned
														? ' · ของฉัน'
														: ''}</Select.Item
												>{/each}
										{:else if activeSection === 'activity'}
											{#each activityGroups as group (group.learningGroupId)}<Select.Item
													value={group.learningGroupId}
													>{group.offeringName} · {group.groupName}{group.assigned
														? ' · ของฉัน'
														: ''}</Select.Item
												>{/each}
										{:else}
											{#each learnerGroups as group (group.learningGroupId)}<Select.Item
													value={group.learningGroupId}
													>{group.code} · {group.name} · {group.groupName}{group.assigned
														? ' · ของฉัน'
														: ''}</Select.Item
												>{/each}
										{/if}
									</Select.Content>
								</Select.Root>
							</div>
						{/if}
					</Card.Content>
				</Card.Root>

				{#if canReadResult && activeSection !== 'learner'}
					<Card.Root class="gap-3 py-4">
						<Card.Header class="px-4"
							><Card.Title class="text-base">เกณฑ์ที่ใช้อยู่</Card.Title><Card.Description
								>แสดงให้อ่านอย่างเดียวในหน้าครู</Card.Description
							></Card.Header
						>
						<Card.Content class="px-4" aria-busy={policyLoading}>
							{#if policyLoading && !policyLoaded}
								<PageSkeleton variant="form" rows={2} />
							{:else if policyError && !policyLoaded}
								<PageState
									variant="error"
									title="โหลดเกณฑ์ตัดผลไม่สำเร็จ"
									description={policyError}
									actionLabel="ลองอีกครั้ง"
									onaction={() => void loadPolicy()}
								/>
							{:else}
								{#if policyLoading}<RegionUpdatingState
										class="static"
										label="กำลังอัปเดตเกณฑ์ตัดผล..."
									/>{/if}
								{#if policyError}<div
										role="alert"
										class="mb-2 flex items-center gap-2 text-sm text-destructive"
									>
										<span>{policyError}</span><Button
											variant="outline"
											size="sm"
											onclick={() => void loadPolicy()}>ลองใหม่</Button
										>
									</div>{/if}
								{#if activePolicy}
									<p class="font-semibold">{activePolicy.name}</p>
									<p class="text-sm text-muted-foreground">รุ่นที่ {activePolicy.versionNo}</p>
								{:else}<p class="text-sm text-amber-700">ยังไม่มีเกณฑ์ตัดผลที่เปิดใช้งาน</p>{/if}
							{/if}
						</Card.Content>
					</Card.Root>
				{/if}
			</div>

			{#if workspaceLoading}
				<PageSkeleton variant="table" rows={8} columns={4} />
			{:else if workspaceError}
				<PageState
					variant="error"
					title="โหลดข้อมูลกลุ่มไม่สำเร็จ"
					description={workspaceError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadSelectedWorkspace()}
				/>
			{:else if errorMessage && !selectedGroupId}
				<PageState
					variant="error"
					title="โหลดข้อมูลสรุปผลไม่สำเร็จ"
					description={errorMessage}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadOverview()}
				/>
			{:else if !selectedGroupId}
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'result-preparation-groups',
						status: 'missing',
						title: 'ยังไม่มีกลุ่มสำหรับเตรียมผล',
						description:
							'ตรวจการกรอกและยืนยันคะแนนหรือผลประเมินรายห้องในสมุดบันทึกก่อนเตรียมผลการเรียน',
						actionLabel: 'ไปกรอกและยืนยันข้อมูล',
						href: '/staff/academic/gradebook'
					}}
				/>
			{:else if activeSection === 'course' && courseWorkspace}
				<ResultPreparationTable
					workspace={courseWorkspace}
					{busyStudentId}
					{confirming}
					onselect={(studentId, selection, rowVersion) =>
						void selectCourseOutcome(studentId, selection, rowVersion)}
					onconfirm={() => void confirmCourse()}
				/>
				{#if courseWorkspace.confirmationIsCurrent}
					<div
						class="rounded-xl border border-emerald-200 bg-emerald-50 p-4 text-sm text-emerald-900"
					>
						<p class="flex items-center gap-2 font-medium">
							<CheckCircle2 class="size-4" /> ห้องนี้ยืนยันแล้ว
						</p>
						<p class="mt-1">
							เมื่อทุกห้องของรหัสวิชายืนยันครบ รายวิชาจะพร้อมส่งฝ่ายวิชาการอัตโนมัติ
							โดยไม่ต้องกดส่งซ้ำ
						</p>
					</div>
				{/if}
			{:else if activeSection === 'activity' && activityWorkspace}
				<ActivityEvaluationTable
					workspace={activityWorkspace}
					{busyStudentId}
					{confirming}
					onselect={(studentId, outcome, rowVersion) =>
						void selectActivityOutcome(studentId, outcome, rowVersion)}
					onconfirm={() => void confirmActivity()}
				/>
			{:else if activeSection === 'learner'}
				<div class="grid gap-3 md:grid-cols-2">
					{#each domains as domain (domain)}
						{@const workspace = evaluationWorkspaces[domain]}
						<Card.Root class="gap-3 py-4"
							><Card.Content class="flex items-center gap-3 px-4"
								><div
									class="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary"
								>
									{#if domain === 'desirable_characteristic'}<ShieldCheck
											class="size-5"
										/>{:else}<BookOpenCheck class="size-5" />{/if}
								</div>
								<div class="min-w-0 flex-1">
									<p class="font-medium">
										{domain === 'desirable_characteristic'
											? 'คุณลักษณะอันพึงประสงค์'
											: 'การอ่าน คิดวิเคราะห์ และเขียน'}
									</p>
									<p class="text-sm text-muted-foreground">
										{workspace?.confirmationIsCurrent
											? 'กลุ่มนี้ยืนยันแล้ว'
											: 'กลุ่มนี้ยังไม่ยืนยันครบ'}
									</p>
								</div>
								<Badge variant={workspace?.confirmationIsCurrent ? 'secondary' : 'outline'}
									>{workspace?.locked
										? 'ล็อกแล้ว'
										: workspace?.confirmationIsCurrent
											? 'พร้อม'
											: 'ยังไม่พร้อม'}</Badge
								></Card.Content
							></Card.Root
						>
					{/each}
				</div>
				<div class="rounded-xl border bg-card p-4">
					<div class="space-y-1.5 sm:max-w-md">
						<Label for="summary-student">ดูผลสรุปของนักเรียน</Label><Select.Root
							type="single"
							value={selectedStudentId}
							onValueChange={(value) => void changeStudent(value)}
							><Select.Trigger id="summary-student" class="w-full"
								>{learnerStudents.find(
									(student) => student.studentAcademicYearId === selectedStudentId
								)?.displayName ?? 'เลือกนักเรียน'}</Select.Trigger
							><Select.Content
								>{#each learnerStudents as student (student.studentAcademicYearId)}<Select.Item
										value={student.studentAcademicYearId}>{student.displayName}</Select.Item
									>{/each}</Select.Content
							></Select.Root
						>
					</div>
				</div>
				{#if summaryLoading}<PageSkeleton
						variant="table"
						rows={4}
						columns={2}
					/>{:else if summaryError}<PageState
						variant="error"
						title="ยังดูผลสรุปไม่ได้"
						description={summaryError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => void loadLearnerSummary(selectedStudentId)}
					/>{:else if learnerSummary}<LearnerEvaluationSummary
						summary={learnerSummary}
						subjectLabels={learnerSubjectLabels}
					/>{/if}
			{/if}
		</div>
	{/if}
</PageShell>
