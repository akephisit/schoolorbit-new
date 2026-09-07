<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		getAcademicResultReadiness,
		lockActivityGroupResults,
		lockAllReadyActivityResults,
		lockCourseSubjectResults,
		type AcademicResultReadiness
	} from '$lib/api/academicResults';
	import {
		listLearnerEvaluationSubjects,
		lockLearnerEvaluationSubject,
		type LearnerEvaluationDomain,
		type LearnerEvaluationLockOutcome,
		type LearnerEvaluationSubject
	} from '$lib/api/academicLearnerEvaluations';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import ResultLockQueue, {
		type LearnerEvaluationLockRow
	} from '$lib/components/academic/results/ResultLockQueue.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const request = new LatestRequest();
	const emptyReadiness: AcademicResultReadiness = { courses: [], activities: [] };
	const domains: LearnerEvaluationDomain[] = [
		'desirable_characteristic',
		'reading_thinking_writing'
	];

	let readiness = $state.raw<AcademicResultReadiness>(emptyReadiness);
	let learnerSubjects = $state.raw<LearnerEvaluationSubject[]>([]);
	let learnerOutcomes = $state.raw<Record<string, LearnerEvaluationLockOutcome>>({});
	let loading = $state(false);
	let errorMessage = $state('');
	let busyKey = $state('');

	const academicYearId = $derived($academicContext.selected.academicYearId);
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const canLockCourseResults = $derived($can.has(PERMISSIONS.ACADEMIC_RESULT_LOCK_SCHOOL));
	const canLockLearnerEvaluations = $derived(
		$can.has(PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL)
	);
	const learnerRows = $derived.by(() => {
		const subjects: LearnerEvaluationSubject[] = [];
		for (const subject of learnerSubjects) {
			if (!subjects.some((candidate) => candidate.subjectId === subject.subjectId)) {
				subjects.push(subject);
			}
		}
		return subjects.flatMap((subject) =>
			domains.map(
				(domain): LearnerEvaluationLockRow => ({
					subjectId: subject.subjectId,
					code: subject.code,
					name: subject.name,
					domain,
					outcome: learnerOutcomes[`${subject.subjectId}:${domain}`]
				})
			)
		);
	});

	function contextValue() {
		return academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	}

	async function loadQueue(): Promise<void> {
		const context = contextValue();
		if (!context) return;
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const [nextReadiness, nextSubjects] = await Promise.all([
				canLockCourseResults
					? getAcademicResultReadiness(context, { signal })
					: Promise.resolve(emptyReadiness),
				canLockLearnerEvaluations
					? listLearnerEvaluationSubjects(context, { signal })
					: Promise.resolve([])
			]);
			if (!request.isCurrent(revision)) return;
			readiness = nextReadiness;
			learnerSubjects = nextSubjects;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดคิวล็อกผลไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function lockCourse(subjectId: string): Promise<void> {
		if (!canLockCourseResults) return;
		const context = contextValue();
		if (!context) return;
		busyKey = `course:${subjectId}`;
		try {
			const outcome = await lockCourseSubjectResults(subjectId, context);
			if (outcome.lock) {
				toast.success('ล็อกผลรายวิชาแล้ว');
				await loadQueue();
			} else {
				toast.error('ข้อมูลบางห้องเปลี่ยนแล้ว กรุณาตรวจเหตุผลในคิว');
				readiness = {
					...readiness,
					courses: readiness.courses.map((subject) =>
						subject.subjectId === subjectId
							? { ...subject, ready: false, groups: outcome.groups }
							: subject
					)
				};
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
			if (outcome.lock) {
				toast.success('ล็อกผลกิจกรรมแล้ว');
				await loadQueue();
			} else {
				toast.error(outcome.blockers.map((blocker) => blocker.code).join(', '));
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
			toast.success(
				`ล็อกแล้ว ${outcome.locked.length} กลุ่ม · ข้าม ${outcome.skipped.length} กลุ่ม`
			);
			await loadQueue();
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
			learnerOutcomes = { ...learnerOutcomes, [key]: outcome };
			if (outcome.lock) toast.success('ล็อกผลประเมินด้านนี้แล้ว');
			else toast.error('ยังล็อกไม่ได้ กรุณาตรวจเหตุผลที่แสดงในรายการ');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ล็อกผลประเมินไม่สำเร็จ');
		} finally {
			busyKey = '';
		}
	}

	onMount(() => {
		let loadedContextKey = '';
		const unsubscribe = academicContext.subscribe((state) => {
			const contextKey =
				state.selected.academicYearId && state.selected.academicTermId
					? `${state.selected.academicYearId}:${state.selected.academicTermId}`
					: '';
			if (contextKey && contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				learnerOutcomes = {};
				void loadQueue();
			} else if (!contextKey) {
				loadedContextKey = '';
				request.abort();
				readiness = emptyReadiness;
				learnerSubjects = [];
			}
		});
		return () => {
			unsubscribe();
			request.abort();
		};
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
	{:else if loading}
		<div class="space-y-4">
			<PageSkeleton variant="form" rows={1} /><PageSkeleton variant="table" rows={8} columns={4} />
		</div>
	{:else if errorMessage}
		<PageState
			variant="error"
			title="โหลดคิวล็อกผลไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => void loadQueue()}
		/>
	{:else}
		<ResultLockQueue
			{readiness}
			{learnerRows}
			{busyKey}
			{canLockCourseResults}
			{canLockLearnerEvaluations}
			onlockcourse={(subjectId) => void lockCourse(subjectId)}
			onlockactivity={(groupId) => void lockActivity(groupId)}
			onlockallactivities={() => void lockAllActivities()}
			onlockevaluation={(subjectId, domain) => void lockEvaluation(subjectId, domain)}
		/>
	{/if}
</PageShell>
