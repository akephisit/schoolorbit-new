<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getLeaveCoverageCandidates,
		getLeaveCoverageReturnLessons,
		saveLeaveCoverage,
		refreshLeaveCoverage,
		type LeaveRequestDetail,
		type LeaveCoverageLesson,
		type LeaveCoverageSelection,
		type LeaveCoverageCandidate,
		type LeaveCoverageCandidates,
		type DatedTeachingLesson
	} from '#lib/api/staff-leave.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import {
		coverageSelections,
		coverageByOriginalTeacher,
		coverageCandidateBatch,
		coverageCandidateLessonLimit,
		coverageLessonLabels,
		coverageCounts,
		coverageSelectionMatches,
		coverageReturnCaption,
		coverageStateLabels,
		coverageConsentLabels
	} from '#lib/staff-leave/coverage.js';
	import { leaveDate } from '#lib/staff-leave/presentation.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import DatePicker from '#lib/components/ui/date-picker/DatePicker.svelte';
	import Choice from './Choice.svelte';
	let {
		detail,
		onsaved,
		onrefresh,
		ondirty,
		readonly = false
	}: {
		detail: LeaveRequestDetail;
		onsaved?: (value: LeaveRequestDetail) => void;
		onrefresh?: () => void;
		ondirty?: (value: boolean) => void;
		readonly?: boolean;
	} = $props();
	const plan = $derived(detail.coverage);
	const lessons = $derived(plan.days.flatMap((day) => day.lessons));
	const teacherGroups = $derived(coverageByOriginalTeacher(plan, detail.request));
	let assignments = $state<LeaveCoverageSelection[]>(
		untrack(() => coverageSelections(detail.coverage))
	);
	let baseRevision = $state(untrack(() => detail.request.revision));
	let sourceDigest = $state(untrack(() => detail.coverage.sourceDigest));
	let selected = $state<string[]>([]),
		dirty = $state(false),
		pending = $state(false),
		refreshing = $state(false),
		refreshError = $state(''),
		error = $state('');
	let candidates = $state<LeaveCoverageCandidates | null>(null),
		candidateLoading = $state(false),
		candidateError = $state('');
	let search = $state(''),
		subjectGroupId = $state(''),
		gradeLevelId = $state(''),
		sameSubjectGroupOnly = $state(false),
		sameStudentGroupOnly = $state(false),
		sameGradeOnly = $state(false);
	let choiceLabels = $state<Record<string, { teacherName: string; offeringName: string }>>({});
	let teacher = $state<LeaveCoverageCandidate | null>(null),
		mode = $state<LeaveCoverageSelection['mode']>('substitution'),
		taughtContent = $state('original'),
		taughtActivity = $state('');
	let returnDate = $state<string | undefined>(),
		returnLessons = $state<DatedTeachingLesson[]>([]),
		returnLessonKey = $state(''),
		returnLoading = $state(false),
		returnError = $state('');
	let disposed = false;
	const candidateRequest = new LatestRequest(),
		returnRequest = new LatestRequest(),
		intent = new LeaveMutationIntent(),
		refreshIntent = new LeaveMutationIntent();
	const stale = $derived(
		baseRevision !== detail.request.revision || sourceDigest !== plan.sourceDigest
	);
	const busy = $derived(pending || refreshing);
	const allowed = $derived(!readonly && (detail.canEdit || plan.canManage));
	const counts = $derived(coverageCounts(lessons, assignments));
	const assignedLessonKeys = $derived(
		new Set(assignments.map((assignment) => assignment.lessonKey))
	);
	const selectedReturnLesson = $derived(
		returnLessons.find((lesson) => lesson.lessonKey === returnLessonKey)
	);
	const selectedOriginalTeacher = $derived(
		teacherGroups.find(
			(group) =>
				group.teacherId === lessons.find((lesson) => lesson.lessonKey === selected[0])?.teacherId
		)
	);
	const canFinalize = $derived(
		allowed && plan.canManage && plan.timetableState === 'ready' && counts.selected === counts.total
	);
	$effect.pre(() => {
		const incoming = detail;
		untrack(() => {
			if (
				baseRevision !== incoming.request.revision ||
				sourceDigest !== incoming.coverage.sourceDigest
			) {
				clearPicker();
				selected = [];
			}
			if (!dirty) {
				assignments = coverageSelections(incoming.coverage);
				baseRevision = incoming.request.revision;
				sourceDigest = incoming.coverage.sourceDigest;
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		candidateRequest.abort();
		returnRequest.abort();
	});
	function clearPicker() {
		candidateRequest.abort();
		resetReturnLookup();
		candidates = null;
		teacher = null;
		candidateError = '';
		candidateLoading = false;
	}
	function resetReturnLookup() {
		returnRequest.abort();
		returnLessons = [];
		returnLessonKey = '';
		returnError = '';
		returnLoading = false;
	}
	function toggle(lesson: LeaveCoverageLesson, checked: boolean) {
		const key = lesson.lessonKey;
		if (checked) {
			if (selectedOriginalTeacher?.teacherId !== lesson.teacherId) selected = [key];
			else if (selected.length < coverageCandidateLessonLimit && !selected.includes(key))
				selected = [...selected, key];
			else return;
		} else {
			selected = selected.filter((value) => value !== key);
		}
		clearPicker();
	}
	function toggleTeacherLessons(keys: string[]) {
		selected = keys.every((key) => selected.includes(key)) ? [] : keys;
		clearPicker();
	}
	async function findCandidates(event?: SubmitEvent) {
		event?.preventDefault();
		if (
			!allowed ||
			!selected.length ||
			selected.length > coverageCandidateLessonLimit ||
			!sourceDigest ||
			stale ||
			busy
		)
			return;
		const ticket = candidateRequest.begin();
		candidateLoading = true;
		candidateError = '';
		candidates = null;
		teacher = null;
		resetReturnLookup();
		try {
			const result = await getLeaveCoverageCandidates(
				detail.request.id,
				{
					expectedRevision: baseRevision,
					sourceDigest,
					lessonKeys: selected,
					search: search.trim() || null,
					subjectGroupId: subjectGroupId || null,
					gradeLevelId: gradeLevelId || null,
					sameSubjectGroupOnly,
					sameStudentGroupOnly,
					sameGradeOnly
				},
				{ signal: ticket.signal }
			);
			if (candidateRequest.isCurrent(ticket.revision) && !disposed) candidates = result;
		} catch (cause) {
			if (candidateRequest.isCurrent(ticket.revision) && !disposed)
				candidateError = cause instanceof Error ? cause.message : 'ค้นหาครูไม่สำเร็จ';
		} finally {
			if (candidateRequest.isCurrent(ticket.revision) && !disposed) candidateLoading = false;
		}
	}
	function choose(candidate: LeaveCoverageCandidate) {
		if (!allowed || busy || disposed) return;
		teacher = candidate;
		mode = 'substitution';
		taughtContent = 'original';
		taughtActivity = '';
		returnDate = undefined;
		resetReturnLookup();
	}
	async function findReturnLessons() {
		if (
			!allowed ||
			busy ||
			!teacher ||
			mode !== 'reciprocal' ||
			!sourceDigest ||
			selected.length !== 1 ||
			!returnDate ||
			stale
		)
			return;
		resetReturnLookup();
		const ticket = returnRequest.begin();
		returnLoading = true;
		try {
			const result = await getLeaveCoverageReturnLessons(
				detail.request.id,
				{
					expectedRevision: baseRevision,
					sourceDigest,
					lessonKey: selected[0],
					replacementTeacherId: teacher.teacherId,
					date: returnDate
				},
				{ signal: ticket.signal }
			);
			if (returnRequest.isCurrent(ticket.revision) && !disposed) returnLessons = result.lessons;
		} catch (cause) {
			if (returnRequest.isCurrent(ticket.revision) && !disposed)
				returnError = cause instanceof Error ? cause.message : 'ค้นหาคาบคืนไม่สำเร็จ';
		} finally {
			if (returnRequest.isCurrent(ticket.revision) && !disposed) returnLoading = false;
		}
	}
	function apply() {
		if (!teacher || !allowed || stale || busy) return;
		const selectedTeacher = teacher;
		const back = mode === 'reciprocal' ? selectedReturnLesson : undefined;
		if (mode === 'reciprocal' && (!back || selected.length !== 1)) {
			candidateError = 'เลือกคาบสอนคืนสำหรับนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน';
			return;
		}
		const useActivity = mode === 'substitution' && taughtContent === 'activity';
		const activity = taughtActivity.trim();
		if (useActivity && !activity) {
			candidateError = 'ระบุกิจกรรมที่สอนจริง';
			return;
		}
		const taughtOfferingId = back
			? (back.offeringId ?? null)
			: taughtContent === 'original' || useActivity
				? null
				: taughtContent;
		const actualActivity =
			back && !taughtOfferingId
				? (back.offeringName ?? 'กิจกรรมตามตาราง')
				: useActivity
					? activity
					: null;
		for (const key of selected)
			choiceLabels[key] = {
				teacherName: teacher.displayName,
				offeringName:
					back?.offeringName ??
					teacher.taughtOfferings.find((value) => value.id === taughtOfferingId)?.name ??
					'วิชาที่เลือก'
			};
		assignments = [
			...assignments.filter((value) => !selected.includes(value.lessonKey)),
			...selected.map((lessonKey) => ({
				lessonKey,
				replacementTeacherId: selectedTeacher.teacherId,
				mode,
				taughtOfferingId,
				taughtActivity: actualActivity,
				returnLessonKey: back?.lessonKey ?? null
			}))
		];
		dirty = true;
		ondirty?.(true);
		selected = [];
		clearPicker();
	}
	async function save() {
		if (!allowed || stale || !sourceDigest || busy) return;
		pending = true;
		error = '';
		const basis = { expectedRevision: baseRevision, sourceDigest, assignments };
		try {
			const result = await saveLeaveCoverage(detail.request.id, {
				requestId: intent.for(basis),
				...basis
			});
			if (!disposed) acceptAuthoritative(result);
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกแผนไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
	function acceptAuthoritative(result: LeaveRequestDetail) {
		dirty = false;
		ondirty?.(false);
		baseRevision = result.request.revision;
		sourceDigest = result.coverage.sourceDigest;
		assignments = coverageSelections(result.coverage);
		selected = [];
		choiceLabels = {};
		clearPicker();
		error = '';
		refreshError = '';
		onsaved?.(result);
	}
	async function refreshCoverage() {
		if (!allowed || busy || disposed) return;
		const basis = { expectedRevision: detail.request.revision };
		refreshing = true;
		refreshError = '';
		candidateRequest.abort();
		candidateLoading = false;
		resetReturnLookup();
		try {
			const result = await refreshLeaveCoverage(detail.request.id, {
				requestId: refreshIntent.for(basis),
				...basis
			});
			if (!disposed) acceptAuthoritative(result);
		} catch (cause) {
			if (!disposed)
				refreshError = cause instanceof Error ? cause.message : 'ปรับคาบตามตารางล่าสุดไม่สำเร็จ';
		} finally {
			if (!disposed) refreshing = false;
		}
	}

	function displayAssignment(key: string) {
		return assignments.find((value) => value.lessonKey === key);
	}
</script>

<section class="space-y-4" data-testid="leave-coverage-editor">
	<div
		class="flex flex-wrap items-start justify-between gap-3 rounded-xl border border-primary/20 bg-primary/5 p-4"
	>
		<div>
			<h2 class="text-lg font-semibold">คาบสอนที่ได้รับผลกระทบ</h2>
			<p class="text-sm text-muted-foreground">
				{coverageStateLabels[plan.state]} · เลือกครู {counts.selected}/{counts.total} · ตอบรับ {counts.confirmed}/{counts.total}
			</p>
		</div>
		{#if dirty}<Badge variant="outline">มีแผนที่ยังไม่บันทึก</Badge>{/if}
	</div>
	{#if allowed}<section
			class="flex flex-col gap-3 rounded-lg border bg-card p-3 sm:flex-row sm:items-center sm:justify-between"
			aria-label="ปรับคาบจากตารางล่าสุด"
		>
			<p class="min-w-0 flex-1 text-sm text-muted-foreground">
				ใช้แผนที่บันทึกไว้กับคาบล่าสุด และแทนที่ตัวเลือกคาบที่ยังไม่บันทึก
			</p>
			<LoadingButton
				variant="outline"
				loading={refreshing}
				loadingLabel="กำลังปรับคาบ..."
				disabled={busy}
				onclick={refreshCoverage}>ปรับคาบตามตารางล่าสุด</LoadingButton
			>
		</section>{/if}
	{#if refreshError}<PageState
			variant="error"
			title="ปรับคาบตามตารางล่าสุดไม่สำเร็จ"
			description={refreshError}
			actionLabel={onrefresh ? 'ตรวจสอบคำขอล่าสุด' : undefined}
			onaction={onrefresh}
		/>{/if}

	{#if plan.blockingReason}<PageState
			variant="error"
			title="ยังจัดคาบสอนไม่ได้"
			description={plan.blockingReason}
		/>
	{:else if plan.timetableState !== 'ready'}<PageState
			title={plan.timetableState === 'missing'
				? 'ยังไม่มีตารางสอนสำหรับช่วงที่เลือก'
				: 'ตารางเดิมยังตรวจสอบไม่ได้'}
			description="ตรวจสอบปี ภาคเรียน และการตั้งค่าตารางสอนกับผู้ดูแล การไม่มีตารางสอนไม่ได้หมายความว่าไม่มีคาบ"
		/>
	{:else if !lessons.length}<PageState
			title="ไม่มีคาบสอนที่ได้รับผลกระทบ"
			description="ตรวจสอบตารางในช่วงวันที่และบริบทที่เลือกแล้ว"
		/>{/if}

	{#if stale}<PageState
			variant="error"
			title="ข้อมูลคำขอหรือตารางมีการเปลี่ยนแปลง"
			description="แผนที่แก้ไขยังไม่ผ่านการตรวจตารางล่าสุด ใช้การปรับคาบด้านบนเพื่อเริ่มจากแผนที่บันทึกไว้"
		/>{/if}
	{#if error}<PageState
			variant="error"
			title="บันทึกแผนไม่สำเร็จ"
			description={error}
			actionLabel={onrefresh ? 'ตรวจสอบคำขอล่าสุด' : undefined}
			onaction={onrefresh}
		/>{/if}
	<div class="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(20rem,26rem)]">
		<div class="min-w-0 space-y-4">
			{#if allowed && lessons.length}
				<p class="text-sm text-muted-foreground">
					เลือกคาบของครูเจ้าของคาบทีละคน ครูสอนแทนต้องว่างทุกคาบที่เลือก
				</p>
			{/if}
			{#each teacherGroups as group (group.teacherId)}
				{@const groupKeys = group.days.flatMap((day) =>
					day.lessons.map((lesson) => lesson.lessonKey)
				)}
				{@const batchKeys = coverageCandidateBatch(groupKeys, assignedLessonKeys)}
				{@const oversized = groupKeys.length > coverageCandidateLessonLimit}
				{@const selectedAll = batchKeys.every((key) => selected.includes(key))}
				<section class="space-y-3" aria-label={`คาบของ ${group.name}`}>
					<div class="flex flex-wrap items-center justify-between gap-2">
						<h3 class="min-w-0 break-words font-semibold">{group.name}</h3>
						{#if allowed && batchKeys.length}
							<Button
								variant="outline"
								size="sm"
								disabled={busy || stale || !sourceDigest}
								onclick={() => toggleTeacherLessons(batchKeys)}
								>{selectedAll
									? 'ล้างการเลือก'
									: oversized
										? `เลือก ${batchKeys.length} คาบที่ยังไม่ได้จัด`
										: 'เลือกทุกคาบ'}</Button
							>
						{/if}
					</div>
					{#if allowed && oversized}
						<p class="text-sm text-muted-foreground">
							ค้นหาครูได้ครั้งละไม่เกิน {coverageCandidateLessonLimit} คาบ
						</p>
					{/if}
					{#each group.days as day (day.date)}
						{@const dayCounts = coverageCounts(day.lessons, assignments)}
						<section
							class="overflow-hidden rounded-xl border bg-card"
							aria-label={`คาบสอน ${leaveDate(day.date)}`}
						>
							<div
								class="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/20 px-4 py-3"
							>
								<h4 class="font-semibold">{leaveDate(day.date)}</h4>
								<p class="text-xs text-muted-foreground">
									เลือกครู {dayCounts.selected}/{dayCounts.total} · ตอบรับ {dayCounts.confirmed}/{dayCounts.total}
								</p>
							</div>
							<ul class="divide-y">
								{#each day.lessons as lesson (lesson.lessonKey)}
									{@const assignment = displayAssignment(lesson.lessonKey)}
									{@const labels = coverageLessonLabels(lesson)}
									{@const checked = selected.includes(lesson.lessonKey)}
									<li class="space-y-3 p-4" data-testid="leave-coverage-lesson">
										<div class="flex items-start gap-3">
											{#if allowed}<Checkbox
													id={`coverage-${lesson.lessonKey}`}
													aria-label={`เลือก ${day.date} ${lesson.periodName}`}
													{checked}
													disabled={busy ||
														stale ||
														!sourceDigest ||
														(!checked &&
															group.teacherId === selectedOriginalTeacher?.teacherId &&
															selected.length >= coverageCandidateLessonLimit)}
													onCheckedChange={(value) => toggle(lesson, value)}
												/>{/if}
											<div class="min-w-0 flex-1">
												<p class="font-medium">
													{lesson.periodName} · {lesson.startTime.slice(
														0,
														5
													)}–{lesson.endTime.slice(0, 5)}
												</p>
												<p class="break-words">{labels.title}</p>
												{#if labels.details}<p class="text-sm text-muted-foreground">
														{labels.details}
													</p>{/if}
											</div>
										</div>
										{#if assignment}<div class="space-y-1 rounded-lg bg-muted/30 p-3 text-sm">
												<p class="font-medium">
													{lesson.assignment?.replacementTeacherId ===
													assignment.replacementTeacherId
														? lesson.assignment.replacementTeacherName
														: (choiceLabels[lesson.lessonKey]?.teacherName ?? 'ครูที่เลือกใหม่')}
												</p>
												<p>
													{assignment.mode === 'reciprocal' ? 'แลกคาบสอน' : 'สอนแทน'} · {assignment.taughtActivity ??
														(assignment.taughtOfferingId
															? ((lesson.assignment?.taughtOfferingId ===
																assignment.taughtOfferingId
																	? lesson.assignment.taughtOfferingName
																	: choiceLabels[lesson.lessonKey]?.offeringName) ?? 'วิชาที่เลือก')
															: (lesson.offeringName ?? 'กิจกรรมตามตาราง'))}
												</p>
												<p class="text-muted-foreground">
													{lesson.assignment && coverageSelectionMatches(lesson, assignment)
														? coverageConsentLabels[lesson.assignment.consent]
														: 'รอบันทึกและตอบรับ'}
												</p>
												{#if lesson.assignment?.returnDate && assignment.returnLessonKey === lesson.assignment.returnLessonKey}<p
													>
														สอน{coverageReturnCaption(
															lesson,
															lesson.assignment,
															leaveDate(lesson.assignment.returnDate)
														)}
													</p>{/if}
												{#if allowed}<Button
														variant="ghost"
														size="sm"
														disabled={busy || stale}
														onclick={() => {
															assignments = assignments.filter(
																(value) => value.lessonKey !== lesson.lessonKey
															);
															dirty = true;
															ondirty?.(true);
														}}>นำครูออกจากคาบนี้</Button
													>{/if}
											</div>{:else}<p class="text-sm text-muted-foreground">
												ยังไม่ได้เลือกครู
											</p>{/if}
									</li>{/each}
							</ul>
						</section>{/each}
				</section>
			{/each}
		</div>
		{#if allowed}<aside
				class="min-w-0 space-y-4 rounded-xl border bg-card p-4 xl:sticky xl:top-4 xl:max-h-[calc(100dvh-10rem)] xl:overflow-y-auto"
				aria-label="ค้นหาครูและกำหนดคาบสอน"
			>
				<h3 class="font-semibold">เลือกครูสำหรับ {selected.length} คาบ</h3>
				{#if selectedOriginalTeacher}<p class="break-words text-sm text-muted-foreground">
						เจ้าของคาบ {selectedOriginalTeacher.name}
					</p>{/if}
				<form onsubmit={findCandidates} class="space-y-3">
					<div class="space-y-2">
						<Label for="coverage-teacher-search">ชื่อครู</Label><Input
							id="coverage-teacher-search"
							bind:value={search}
							disabled={busy}
							placeholder="ค้นหาชื่อครู"
						/>
					</div>
					<Choice
						id="coverage-subject-group"
						label="กลุ่มสาระ"
						bind:value={subjectGroupId}
						options={[
							{ value: '', label: 'ทุกกลุ่มสาระ' },
							...(candidates?.subjectGroups ?? []).map((value) => ({
								value: value.id,
								label: value.name
							}))
						]}
						disabled={busy || candidateLoading}
					/>
					<Choice
						id="coverage-grade"
						label="ระดับชั้นที่สอนจริง"
						bind:value={gradeLevelId}
						options={[
							{ value: '', label: 'ทุกระดับชั้น' },
							...(candidates?.gradeLevels ?? []).map((value) => ({
								value: value.id,
								label: value.name
							}))
						]}
						disabled={busy || candidateLoading}
					/>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-subject"
							bind:checked={sameSubjectGroupOnly}
							disabled={busy || candidateLoading}
						/><Label for="coverage-same-subject">เฉพาะกลุ่มสาระเดียวกัน</Label>
					</div>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-students"
							bind:checked={sameStudentGroupOnly}
							disabled={busy || candidateLoading}
						/><Label for="coverage-same-students">สอนนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน</Label>
					</div>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-grade"
							bind:checked={sameGradeOnly}
							disabled={busy || candidateLoading}
						/><Label for="coverage-same-grade">สอนระดับชั้นเดียวกัน</Label>
					</div>

					<LoadingButton
						type="submit"
						loading={candidateLoading}
						disabled={!selected.length || !sourceDigest || stale || busy}
						>ค้นหาครูที่ว่างทุกคาบ</LoadingButton
					>
				</form>
				{#if candidateError}<PageState
						variant="error"
						title="เลือกครูไม่สำเร็จ"
						description={candidateError}
					/>{/if}
				{#if candidateLoading}<PageSkeleton variant="cards" />{:else if candidates}<div
						class="space-y-2"
					>
						<p class="text-xs text-muted-foreground">
							เปิดให้เลือกข้ามกลุ่มสาระได้ · แสดงเฉพาะครูที่ว่างทุกคาบ
						</p>
						<ul class="max-h-80 space-y-2 overflow-y-auto" aria-label="ครูที่ว่าง">
							{#each candidates.candidates as candidate (candidate.teacherId)}<li>
									<Button
										class="h-auto w-full justify-start whitespace-normal text-left"
										variant={teacher?.teacherId === candidate.teacherId ? 'secondary' : 'outline'}
										disabled={busy}
										onclick={() => choose(candidate)}
										><span class="min-w-0"
											><span class="block break-words font-medium">{candidate.displayName}</span
											><span class="block text-xs text-muted-foreground"
												>{candidate.subjectGroups.map((value) => value.name).join(', ')} · {candidate.gradeLevels
													.map((value) => value.name)
													.join(', ')}</span
											></span
										></Button
									>
								</li>{/each}
						</ul>
						{#if !candidates.candidates.length}<p class="text-sm text-muted-foreground">
								ไม่มีครูที่ว่างตามเงื่อนไข ลองปรับตัวกรอง
							</p>{/if}{#if candidates.hasMore}<p class="text-xs text-muted-foreground">
								มีผลลัพธ์เพิ่มเติม ระบุชื่อหรือตัวกรองให้แคบลง
							</p>{/if}
					</div>{/if}
				{#if teacher}<div class="space-y-3 border-t pt-4">
						<h4 class="font-medium">{teacher.displayName}</h4>
						<Choice
							id="coverage-mode"
							disabled={busy}
							label="วิธีจัดคาบ"
							value={mode}
							onchange={(value) => {
								mode = value === 'reciprocal' ? 'reciprocal' : 'substitution';
								taughtContent = 'original';
								taughtActivity = '';
								candidateError = '';
								resetReturnLookup();
							}}
							options={[
								{ value: 'substitution', label: 'สอนแทน' },
								...(selected.length === 1 ? [{ value: 'reciprocal', label: 'แลกคาบสอน' }] : [])
							]}
						/>
						{#if mode === 'substitution'}
							<Choice
								id="coverage-taught-offering"
								disabled={busy}
								label="เนื้อหาที่สอนจริง"
								bind:value={taughtContent}
								onchange={() => (candidateError = '')}
								options={[
									{ value: 'original', label: 'สอนตามคาบเดิม' },
									...teacher.taughtOfferings.map((value) => ({
										value: value.id,
										label: value.name
									})),
									{ value: 'activity', label: 'ระบุกิจกรรมแทนวิชา' }
								]}
							/>
							{#if taughtContent === 'activity'}
								<div class="space-y-2">
									<Label for="coverage-activity">กิจกรรมที่สอนจริง</Label>
									<Textarea
										id="coverage-activity"
										disabled={busy}
										bind:value={taughtActivity}
										maxlength={200}
										rows={2}
										placeholder="เช่น ทบทวนภาษาอังกฤษ หรือกิจกรรมอ่านหนังสือ"
									/>
								</div>
							{/if}
						{/if}
						{#if mode === 'reciprocal'}<div class="space-y-3 rounded-lg border p-3">
								<p class="text-sm">
									{selectedOriginalTeacher?.name ?? 'ครูเจ้าของคาบ'} จะสอนคืน เลือกคาบของนักเรียนกลุ่มเดียวกันหรือห้องเดียวกัน
								</p>
								<div class="space-y-2">
									<Label for="coverage-return-date">วันที่สอนคืน</Label><DatePicker
										id="coverage-return-date"
										disabled={busy}
										bind:value={returnDate}
										onValueChange={resetReturnLookup}
									/>
								</div>
								<LoadingButton
									loading={returnLoading}
									disabled={busy || !returnDate}
									onclick={findReturnLessons}>ค้นหาคาบสอนคืน</LoadingButton
								>{#if returnError}<PageState
										variant="error"
										title="ค้นหาคาบคืนไม่สำเร็จ"
										description={returnError}
									/>{/if}{#if returnLessons.length}<Choice
										id="coverage-return-lesson"
										disabled={busy}
										label="คาบสอนคืน"
										bind:value={returnLessonKey}
										options={returnLessons.map((value) => ({
											value: value.lessonKey,
											label: `${value.periodName} ${value.startTime.slice(0, 5)}–${value.endTime.slice(0, 5)} · ${value.offeringName ?? 'กิจกรรม'} · ${value.homeroomNames.join(', ')}${value.roomName ? ' · ' + value.roomName : ''}`
										}))}
									/>{:else if returnDate && !returnLoading && !returnError}<p
										class="text-xs text-muted-foreground"
									>
										ค้นหาวันที่ที่มีคาบคืนสำหรับนักเรียนกลุ่มเดียวกัน
									</p>{/if}
							</div>{/if}
						{#if mode === 'reciprocal' && selectedReturnLesson}<p
								class="rounded-lg bg-primary/5 p-3 text-sm"
							>
								วิชาที่สอนจริง: {selectedReturnLesson.offeringName ?? 'กิจกรรมตามตาราง'} · อ้างอิงจากคาบที่แลก
							</p>{/if}
						<Button disabled={busy || stale} onclick={apply}
							>ใช้กับ {selected.length} คาบที่เลือก</Button
						>
					</div>{/if}
			</aside>{/if}
	</div>
	{#if allowed}<div
			class="sticky bottom-0 flex flex-wrap items-center justify-between gap-3 border-t bg-background py-3"
		>
			<p class="text-sm text-muted-foreground">
				บันทึกแผนที่ยังไม่ครบได้ เมื่อเปลี่ยนรายละเอียดคาบ ระบบจะขอให้ครูยืนยันใหม่ก่อนพิจารณา
			</p>
			<LoadingButton
				loading={pending}
				disabled={busy || (!dirty && !canFinalize) || stale || !sourceDigest}
				onclick={save}>{canFinalize ? 'บันทึกแผนและส่งพิจารณา' : 'บันทึกแผนคาบสอน'}</LoadingButton
			>
		</div>{/if}
</section>
