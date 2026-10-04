<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicResultReadiness,
		lockActivityGroupResults,
		lockAllReadyActivityResults,
		lockCourseSubjectResults,
		type AcademicResultReadiness
	} from '#lib/api/academicResults.js';
	import {
		getLearnerEvaluationLockReadiness,
		lockLearnerEvaluationSubject,
		type LearnerEvaluationDomain,
		type LearnerEvaluationSubjectLockReadiness
	} from '#lib/api/academicLearnerEvaluations.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import ResultLockQueue from '#lib/components/academic/results/ResultLockQueue.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	let { data }: PageProps = $props();
	const request = new LatestRequest();
	const emptyReadiness: AcademicResultReadiness = { courses: [], activities: [] };

	let readiness = $state.raw<AcademicResultReadiness>(emptyReadiness);
	let learnerRows = $state.raw<LearnerEvaluationSubjectLockReadiness[]>([]);
	let activeTab = $state<'course' | 'activity' | 'learner'>('course');
	let readinessLoaded = $state(false);
	let learnerLoaded = $state(false);
	let loading = $state(true);
	let loadingDomain = $state<'result' | 'learner' | null>(null);
	let errorMessage = $state('');
	let busyKey = $state('');

	const academicYearId = $derived(data.context?.academicYearId ?? null);
	const academicTermId = $derived(data.context?.academicTermId ?? null);
	const canLockCourseResults = $derived($can.has(PERMISSIONS.ACADEMIC_RESULT_LOCK_SCHOOL));
	const canLockLearnerEvaluations = $derived(
		$can.has(PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL)
	);
	function contextValue() {
		return academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	}

	function syncUrl(): void {
		const url = new URL(page.url.href);
		url.searchParams.set('tab', activeTab);
		goto(resolve(`staff/academic/result-locks?${url.searchParams.toString()}`), {
			shallow: true,
			replace: true,
			state: page.state
		});
	}

	function changeTab(value: 'course' | 'activity' | 'learner'): void {
		if (value === activeTab) return;
		if (value === 'learner' ? !canLockLearnerEvaluations : !canLockCourseResults) return;
		activeTab = value;
		syncUrl();
		if (value === 'learner' ? learnerLoaded : readinessLoaded) {
			request.abort();
			loading = false;
			loadingDomain = null;
			errorMessage = '';
		} else if (loadingDomain !== (value === 'learner' ? 'learner' : 'result')) void loadQueue();
	}

	async function loadQueue(): Promise<void> {
		const context = contextValue();
		if (!context) return;
		const { revision, signal } = request.begin();
		loading = true;
		loadingDomain = activeTab === 'learner' ? 'learner' : 'result';
		errorMessage = '';
		try {
			const next =
				activeTab === 'learner'
					? await getLearnerEvaluationLockReadiness(context, { signal })
					: await getAcademicResultReadiness(context, { signal });
			if (!request.isCurrent(revision)) return;
			if (activeTab === 'learner') {
				learnerRows = next as LearnerEvaluationSubjectLockReadiness[];
				learnerLoaded = true;
			} else {
				readiness = next as AcademicResultReadiness;
				readinessLoaded = true;
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดคิวล็อกผลไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) {
				loading = false;
				loadingDomain = null;
			}
		}
	}

	async function lockCourse(subjectId: string): Promise<void> {
		if (!canLockCourseResults) return;
		const context = contextValue();
		if (!context) return;
		busyKey = `course:${subjectId}`;
		try {
			const outcome = await lockCourseSubjectResults(subjectId, context);
			if (context.academicTermId !== academicTermId || context.academicYearId !== academicYearId)
				return;
			readiness = {
				...readiness,
				courses: readiness.courses.map((subject) =>
					subject.subjectId === subjectId
						? {
								...subject,
								ready: outcome.groups.every((group) => group.ready),
								groups: outcome.lock
									? outcome.groups.map((group) => ({ ...group, locked: true }))
									: outcome.groups
							}
						: subject
				)
			};
			if (outcome.lock) {
				toast.success('ล็อกผลรายวิชาแล้ว');
			} else {
				toast.error('ข้อมูลบางห้องเปลี่ยนแล้ว กรุณาตรวจเหตุผลในคิว');
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล็อกผลรายวิชาไม่สำเร็จ');
		} finally {
			busyKey = '';
		}
	}

	async function lockActivity(groupId: string): Promise<void> {
		if (!canLockCourseResults) return;
		const context = contextValue();
		if (!context) return;
		busyKey = `activity:${groupId}`;
		try {
			const outcome = await lockActivityGroupResults(groupId, context);
			if (context.academicTermId !== academicTermId || context.academicYearId !== academicYearId)
				return;
			if (outcome.lock) {
				toast.success('ล็อกผลกิจกรรมแล้ว');
				readiness = {
					...readiness,
					activities: readiness.activities.map((group) =>
						group.learningGroupId === groupId
							? { ...group, locked: true, ready: true, blockers: [] }
							: group
					)
				};
			} else {
				toast.error(outcome.blockers.map((blocker) => blocker.code).join(', '));
				readiness = {
					...readiness,
					activities: readiness.activities.map((group) =>
						group.learningGroupId === groupId
							? { ...group, ready: false, blockers: outcome.blockers }
							: group
					)
				};
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล็อกผลกิจกรรมไม่สำเร็จ');
		} finally {
			busyKey = '';
		}
	}

	async function lockAllActivities(): Promise<void> {
		if (!canLockCourseResults) return;
		const context = contextValue();
		if (!context) return;
		busyKey = 'activity:all';
		try {
			const outcome = await lockAllReadyActivityResults(context);
			if (context.academicTermId !== academicTermId || context.academicYearId !== academicYearId)
				return;
			toast.success(
				`ล็อกแล้ว ${outcome.locked.length} กลุ่ม · ข้าม ${outcome.skipped.length} กลุ่ม`
			);
			const locked = new Set(outcome.locked.map((lock) => lock.learningGroupId));
			const skipped = new Map(outcome.skipped.map((group) => [group.learningGroupId, group]));
			readiness = {
				...readiness,
				activities: readiness.activities.map((group) =>
					locked.has(group.learningGroupId)
						? { ...group, locked: true, ready: true, blockers: [] }
						: (skipped.get(group.learningGroupId) ?? group)
				)
			};
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล็อกผลกิจกรรมไม่สำเร็จ');
		} finally {
			busyKey = '';
		}
	}

	async function lockEvaluation(subjectId: string, domain: LearnerEvaluationDomain): Promise<void> {
		if (!canLockLearnerEvaluations) return;
		const context = contextValue();
		if (!context) return;
		const key = `${subjectId}:${domain}`;
		busyKey = `learner:${key}`;
		try {
			const outcome = await lockLearnerEvaluationSubject(subjectId, domain, context);
			if (context.academicTermId !== academicTermId || context.academicYearId !== academicYearId)
				return;
			if (outcome.lock) {
				learnerRows = learnerRows.map((row) => {
					if (row.subjectId !== subjectId || row.domain !== domain) return row;
					return {
						...row,
						locked: true,
						ready: true,
						groups: row.groups.map((group) => ({ ...group, ready: true, blockers: [] }))
					};
				});
				toast.success('ล็อกผลประเมินด้านนี้แล้ว');
			} else {
				const refreshed = await getLearnerEvaluationLockReadiness(context);
				if (
					context.academicTermId === academicTermId &&
					context.academicYearId === academicYearId
				) {
					learnerRows = refreshed;
					learnerLoaded = true;
				}
				toast.error('ข้อมูลบางห้องเปลี่ยนแล้ว กรุณาตรวจเหตุผลในคิว');
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล็อกผลประเมินไม่สำเร็จ');
		} finally {
			busyKey = '';
		}
	}

	onDestroy(() => request.abort());
	$effect.pre(() => {
		const routeQueue = data.queue;
		const routeTab = data.tab;
		const { revision } = request.begin();
		untrack(() => {
			activeTab = routeTab;
			readiness = emptyReadiness;
			learnerRows = [];
			readinessLoaded = false;
			learnerLoaded = false;
			loading = Boolean(routeQueue);
			loadingDomain = routeQueue ? (routeTab === 'learner' ? 'learner' : 'result') : null;
			errorMessage = '';
		});
		if (routeQueue) {
			void routeQueue.then((result) => {
				if (!request.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						if (result.data.tab === 'learner') {
							learnerRows = result.data.rows;
							learnerLoaded = true;
						} else {
							readiness = result.data.readiness;
							readinessLoaded = true;
						}
					} else errorMessage = result.error;
					loading = false;
					loadingDomain = null;
				});
			});
		}
		return () => {
			if (request.isCurrent(revision)) request.abort();
		};
	});

	$effect(() => {
		if (!academicTermId) return;
		if (!canLockCourseResults && canLockLearnerEvaluations && activeTab !== 'learner')
			changeTab('learner');
		else if (canLockCourseResults && !canLockLearnerEvaluations && activeTab === 'learner')
			changeTab('course');
	});
</script>

<PageShell
	title="ล็อกผลการเรียน"
	description="ตรวจความพร้อมและสร้างผลเริ่มต้นที่แก้ทับไม่ได้สำหรับฝ่ายวิชาการ"
>
	{#if !canLockCourseResults && !canLockLearnerEvaluations}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ล็อกผลการเรียน"
			description="หน้านี้ใช้สำหรับฝ่ายวิชาการที่ได้รับสิทธิ์ล็อกผลระดับโรงเรียน"
		/>
	{:else if !academicYearId || !academicTermId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="ใช้ตัวเลือกบนแถบด้านบนเพื่อเปิดคิวของภาคเรียนที่ต้องการ"
		/>
	{:else}
		<ResultLockQueue
			{readiness}
			{learnerRows}
			resultsHref={`/staff/academic/results?academicYearId=${academicYearId}&academicTermId=${academicTermId}`}
			{activeTab}
			{loading}
			error={errorMessage}
			{busyKey}
			{canLockCourseResults}
			{canLockLearnerEvaluations}
			onlockcourse={(subjectId) => void lockCourse(subjectId)}
			onlockactivity={(groupId) => void lockActivity(groupId)}
			onlockallactivities={() => void lockAllActivities()}
			onlockevaluation={(subjectId, domain) => void lockEvaluation(subjectId, domain)}
			ontabchange={changeTab}
			onretry={() => void loadQueue()}
		/>
	{/if}
</PageShell>
