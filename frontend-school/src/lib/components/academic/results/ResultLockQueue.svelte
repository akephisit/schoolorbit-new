<script lang="ts">
	import { learnerEvaluationLockBlockerLabel } from '#lib/academic/learner-evaluation/presentation.js';
	import { isSubjectReady, resultBlockerLabel } from '#lib/academic/results/presentation.js';
	import type { AcademicResultReadiness } from '#lib/api/academicResults.js';
	import type {
		LearnerEvaluationDomain,
		LearnerEvaluationSubjectLockReadiness
	} from '#lib/api/academicLearnerEvaluations.js';
	import AcademicPrerequisiteNotice from '#lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { CheckCircle2, CircleAlert, LockKeyhole, ShieldCheck } from '@lucide/svelte';

	let {
		readiness,
		learnerRows,
		resultsHref,
		activeTab,
		loading = false,
		error = '',
		busyKey = '',
		canLockCourseResults,
		canLockLearnerEvaluations,
		onlockcourse,
		onlockactivity,
		onlockallactivities,
		onlockevaluation,
		ontabchange,
		onretry
	}: {
		readiness: AcademicResultReadiness;
		learnerRows: LearnerEvaluationSubjectLockReadiness[];
		resultsHref: string;
		activeTab: 'course' | 'activity' | 'learner';
		loading?: boolean;
		error?: string;
		busyKey?: string;
		canLockCourseResults: boolean;
		canLockLearnerEvaluations: boolean;
		onlockcourse: (subjectId: string) => void;
		onlockactivity: (groupId: string) => void;
		onlockallactivities: () => void;
		onlockevaluation: (subjectId: string, domain: LearnerEvaluationDomain) => void;
		ontabchange: (tab: 'course' | 'activity' | 'learner') => void;
		onretry: () => void;
	} = $props();

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

<Tabs.Root
	value={activeTab}
	onValueChange={(value) => ontabchange(value as 'course' | 'activity' | 'learner')}
>
	<Tabs.List class="grid h-auto w-full grid-cols-1 gap-1 p-1 sm:grid-cols-3">
		<Tabs.Trigger value="course" disabled={!canLockCourseResults}>รายวิชา</Tabs.Trigger>
		<Tabs.Trigger value="learner" disabled={!canLockLearnerEvaluations}
			>ผลประเมินผู้เรียน</Tabs.Trigger
		>
		<Tabs.Trigger value="activity" disabled={!canLockCourseResults}>กิจกรรม</Tabs.Trigger>
	</Tabs.List>
	{#if loading}
		<div class="mt-4"><PageSkeleton variant="table" rows={8} columns={4} /></div>
	{:else if error}
		<div class="mt-4">
			<PageState
				variant="error"
				title="โหลดคิวล็อกผลไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={onretry}
			/>
		</div>
	{:else}
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
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'result-lock-course',
						status: 'missing',
						title: 'ยังไม่มีรายวิชาในคิวล็อกผล',
						description: 'เตรียมและยืนยันผลรายวิชาก่อนล็อก',
						actionLabel: 'ไปเตรียมผลการเรียน',
						href: resultsHref,
						preload: 'tap'
					}}
				/>
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
							{#if row.groups.some((group) => !group.ready)}
								<ul class="mt-3 space-y-2 text-sm">
									{#each row.groups.filter((group) => !group.ready) as group (group.learningGroupId)}
										<li class="rounded-lg border border-amber-200 bg-amber-50 p-2.5 text-amber-950">
											<p class="flex items-center gap-1.5 font-medium">
												<CircleAlert class="size-3.5" />
												{group.groupName}
											</p>
											<ul class="mt-1 list-disc space-y-0.5 pl-5 text-xs text-amber-800">
												{#each group.blockers as blocker (blocker)}
													<li>{learnerEvaluationLockBlockerLabel(blocker)}</li>
												{/each}
											</ul>
										</li>
									{/each}
								</ul>
							{:else if !row.locked}
								<p class="mt-2 flex items-center gap-1 text-xs text-emerald-700">
									<CheckCircle2 class="size-3" /> ทุกกลุ่มยืนยันครบแล้ว
								</p>
							{/if}
						</div>
						{#if row.locked}
							<Badge variant="secondary"><CheckCircle2 class="size-3" /> ล็อกแล้ว</Badge>
						{:else}
							<LoadingButton
								loading={busyKey === `learner:${row.subjectId}:${row.domain}`}
								disabled={!canLockLearnerEvaluations || !row.ready}
								onclick={() => onlockevaluation(row.subjectId, row.domain)}
							>
								<LockKeyhole class="size-4" /> ล็อกด้านนี้
							</LoadingButton>
						{/if}
					</Card.Content>
				</Card.Root>
			{:else}
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'result-lock-learner',
						status: 'missing',
						title: 'ยังไม่มีรายวิชาสำหรับล็อกผลประเมิน',
						description: 'เตรียมและยืนยันผลประเมินผู้เรียนก่อนล็อก',
						actionLabel: 'ไปเตรียมผลประเมิน',
						href: `${resultsHref}&section=learner`,
						preload: 'tap'
					}}
				/>
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
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'result-lock-activity',
						status: 'missing',
						title: 'ยังไม่มีกิจกรรมในคิวล็อกผล',
						description: 'เตรียมและยืนยันผลกิจกรรมก่อนล็อก',
						actionLabel: 'ไปเตรียมผลกิจกรรม',
						href: `${resultsHref}&section=activity`,
						preload: 'tap'
					}}
				/>
			{/each}
		</Tabs.Content>
	{/if}
</Tabs.Root>
