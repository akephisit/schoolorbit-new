<script lang="ts">
	import { learnerEvaluationLockBlockerLabel } from '$lib/academic/learner-evaluation/presentation';
	import { isSubjectReady, resultBlockerLabel } from '$lib/academic/results/presentation';
	import type { AcademicResultReadiness } from '$lib/api/academicResults';
	import type {
		LearnerEvaluationDomain,
		LearnerEvaluationLockOutcome
	} from '$lib/api/academicLearnerEvaluations';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Tabs from '$lib/components/ui/tabs';
	import { CheckCircle2, CircleAlert, LockKeyhole, ShieldCheck } from 'lucide-svelte';

	export interface LearnerEvaluationLockRow {
		subjectId: string;
		code: string;
		name: string;
		domain: LearnerEvaluationDomain;
		outcome?: LearnerEvaluationLockOutcome;
	}

	let {
		readiness,
		learnerRows,
		busyKey = '',
		canLockCourseResults,
		canLockLearnerEvaluations,
		onlockcourse,
		onlockactivity,
		onlockallactivities,
		onlockevaluation
	}: {
		readiness: AcademicResultReadiness;
		learnerRows: LearnerEvaluationLockRow[];
		busyKey?: string;
		canLockCourseResults: boolean;
		canLockLearnerEvaluations: boolean;
		onlockcourse: (subjectId: string) => void;
		onlockactivity: (groupId: string) => void;
		onlockallactivities: () => void;
		onlockevaluation: (subjectId: string, domain: LearnerEvaluationDomain) => void;
	} = $props();
	let activeTab = $state('course');

	function subjectName(subjectId: string): string {
		const known = learnerRows.find((row) => row.subjectId === subjectId);
		const readinessSubject = readiness.courses.find((row) => row.subjectId === subjectId);
		const fallback = readinessSubject?.groups[0]?.offeringName ?? subjectId;
		return known ? `${known.code} · ${known.name}` : fallback;
	}

	function domainLabel(domain: LearnerEvaluationDomain): string {
		return domain === 'desirable_characteristic'
			? 'คุณลักษณะอันพึงประสงค์'
			: 'การอ่าน คิดวิเคราะห์ และเขียน';
	}
</script>

<Tabs.Root bind:value={activeTab}>
	<Tabs.List class="grid h-auto w-full grid-cols-1 gap-1 p-1 sm:grid-cols-3">
		<Tabs.Trigger value="course">รายวิชา</Tabs.Trigger>
		<Tabs.Trigger value="learner">ผลประเมินผู้เรียน</Tabs.Trigger>
		<Tabs.Trigger value="activity">กิจกรรม</Tabs.Trigger>
	</Tabs.List>

	<Tabs.Content value="course" class="mt-4 space-y-3">
		{#each readiness.courses as subject (subject.subjectId)}
			<Card.Root class="gap-3 py-4">
				<Card.Header class="px-4">
					<div class="flex flex-col gap-3 sm:flex-row sm:items-start">
						<div class="min-w-0 flex-1">
							<Card.Title class="text-base">{subjectName(subject.subjectId)}</Card.Title>
							<Card.Description
								>{subject.groups.length} กลุ่มเรียน · ล็อกพร้อมกันทั้งรหัสวิชา</Card.Description
							>
						</div>
						{#if subject.groups.every((group) => group.locked)}
							<Badge variant="secondary"><CheckCircle2 class="size-3" /> ล็อกแล้ว</Badge>
						{:else}
							<LoadingButton
								loading={busyKey === `course:${subject.subjectId}`}
								disabled={!canLockCourseResults || !isSubjectReady(subject.groups)}
								onclick={() => onlockcourse(subject.subjectId)}
							>
								<LockKeyhole class="size-4" /> ล็อกผลรายวิชา
							</LoadingButton>
						{/if}
					</div>
				</Card.Header>
				<Card.Content class="grid gap-2 px-4 md:grid-cols-2 xl:grid-cols-3">
					{#each subject.groups as group (group.learningGroupId)}
						<div class="rounded-lg border p-3 text-sm">
							<div class="flex items-center justify-between gap-2">
								<p class="font-medium">{group.groupName}</p>
								<Badge variant={group.ready ? 'secondary' : 'outline'}>
									{group.locked ? 'ล็อกแล้ว' : group.ready ? 'พร้อม' : 'ยังไม่พร้อม'}
								</Badge>
							</div>
							{#if group.blockers.length > 0}
								<ul class="mt-2 list-disc space-y-1 pl-5 text-muted-foreground">
									{#each group.blockers as blocker (`${blocker.code}:${blocker.assessmentPhaseId ?? ''}`)}
										<li>{resultBlockerLabel(blocker)}</li>
									{/each}
								</ul>
							{/if}
						</div>
					{/each}
				</Card.Content>
			</Card.Root>
		{:else}
			<p class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground">
				ยังไม่มีรายวิชาในคิวล็อกผล
			</p>
		{/each}
	</Tabs.Content>

	<Tabs.Content value="learner" class="mt-4 space-y-3">
		<div class="rounded-xl border bg-blue-50 p-4 text-sm text-blue-900">
			<div class="flex gap-2">
				<ShieldCheck class="mt-0.5 size-4 shrink-0" />
				<p>ล็อกแยกตามรหัสวิชาและด้านประเมิน ระบบตรวจทุกกลุ่มเรียนอีกครั้งก่อนสร้างผลเริ่มต้น</p>
			</div>
		</div>
		{#each learnerRows as row (`${row.subjectId}:${row.domain}`)}
			<Card.Root class="gap-3 py-4">
				<Card.Content class="flex flex-col gap-3 px-4 sm:flex-row sm:items-center">
					<div class="min-w-0 flex-1">
						<p class="font-semibold">{row.code} · {row.name}</p>
						<p class="text-sm text-muted-foreground">{domainLabel(row.domain)}</p>
						{#if row.outcome?.blockers.length}
							<ul class="mt-2 list-disc space-y-1 pl-5 text-sm text-amber-800">
								{#each row.outcome.blockers as blocker (blocker.learningGroupId)}
									<li>{learnerEvaluationLockBlockerLabel(blocker.reason)}</li>
								{/each}
							</ul>
						{:else if !row.outcome?.lock}
							<p class="mt-2 flex items-center gap-1 text-xs text-muted-foreground">
								<CircleAlert class="size-3" /> ตรวจความครบถ้วนเมื่อกดล็อก
							</p>
						{/if}
					</div>
					{#if row.outcome?.lock}
						<Badge variant="secondary"><CheckCircle2 class="size-3" /> ล็อกแล้ว</Badge>
					{:else}
						<LoadingButton
							loading={busyKey === `learner:${row.subjectId}:${row.domain}`}
							disabled={!canLockLearnerEvaluations}
							onclick={() => onlockevaluation(row.subjectId, row.domain)}
						>
							<LockKeyhole class="size-4" /> ล็อกด้านนี้
						</LoadingButton>
					{/if}
				</Card.Content>
			</Card.Root>
		{:else}
			<p class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground">
				ยังไม่มีรายวิชาสำหรับล็อกผลประเมิน
			</p>
		{/each}
	</Tabs.Content>

	<Tabs.Content value="activity" class="mt-4 space-y-3">
		<div class="flex justify-end">
			<LoadingButton
				variant="outline"
				loading={busyKey === 'activity:all'}
				disabled={!canLockCourseResults || (busyKey !== '' && busyKey !== 'activity:all')}
				onclick={onlockallactivities}
			>
				<LockKeyhole class="size-4" /> ล็อกผลกิจกรรมที่พร้อมทั้งหมด
			</LoadingButton>
		</div>
		{#each readiness.activities as group (group.learningGroupId)}
			<Card.Root class="gap-3 py-4">
				<Card.Content class="flex flex-col gap-3 px-4 sm:flex-row sm:items-start">
					<div class="min-w-0 flex-1">
						<p class="font-semibold">{group.offeringName}</p>
						<p class="text-sm text-muted-foreground">{group.groupName}</p>
						{#if group.blockers.length > 0}
							<ul class="mt-2 list-disc space-y-1 pl-5 text-sm text-muted-foreground">
								{#each group.blockers as blocker (`${blocker.code}:${blocker.studentAcademicYearId ?? ''}`)}
									<li>{resultBlockerLabel(blocker)}</li>
								{/each}
							</ul>
						{/if}
					</div>
					{#if group.locked}
						<Badge variant="secondary"><CheckCircle2 class="size-3" /> ล็อกแล้ว</Badge>
					{:else}
						<LoadingButton
							loading={busyKey === `activity:${group.learningGroupId}`}
							disabled={!canLockCourseResults || !group.ready}
							onclick={() => onlockactivity(group.learningGroupId)}
						>
							<LockKeyhole class="size-4" /> ล็อกผลกลุ่มนี้
						</LoadingButton>
					{/if}
				</Card.Content>
			</Card.Root>
		{:else}
			<p class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground">
				ยังไม่มีกิจกรรมในคิวล็อกผล
			</p>
		{/each}
	</Tabs.Content>
</Tabs.Root>
