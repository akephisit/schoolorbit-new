<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getLeaveCoverageCandidates,
		getLeaveCoverageReturnLessons,
		saveLeaveCoverage,
		type LeaveRequestDetail,
		type LeaveCoverageSelection,
		type LeaveCoverageCandidate,
		type LeaveCoverageCandidates,
		type DatedTeachingLesson
	} from '#lib/api/staff-leave.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LeaveMutationIntent } from '#lib/staff-leave/mutation-intent.js';
	import {
		coverageSelections,
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
	let assignments = $state<LeaveCoverageSelection[]>(
		untrack(() => coverageSelections(detail.coverage))
	);
	let baseRevision = $state(untrack(() => detail.request.revision));
	let sourceDigest = $state(untrack(() => detail.coverage.sourceDigest));
	let selected = $state<string[]>([]),
		dirty = $state(false),
		pending = $state(false),
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
		taughtOfferingId = $state(''),
		taughtActivity = $state('');
	let returnDate = $state<string | undefined>(),
		returnLessons = $state<DatedTeachingLesson[]>([]),
		returnLessonKey = $state(''),
		returnLoading = $state(false),
		returnError = $state('');
	let disposed = false;
	const candidateRequest = new LatestRequest(),
		returnRequest = new LatestRequest(),
		intent = new LeaveMutationIntent();
	const stale = $derived(
		baseRevision !== detail.request.revision || sourceDigest !== plan.sourceDigest
	);
	const allowed = $derived(!readonly && (detail.canEdit || plan.canManage));
	const counts = $derived(coverageCounts(lessons, assignments));
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
		returnRequest.abort();
		candidates = null;
		teacher = null;
		candidateError = '';
		returnLessons = [];
		returnLessonKey = '';
		returnError = '';
		candidateLoading = false;
		returnLoading = false;
	}
	function toggle(key: string, checked: boolean) {
		selected = checked
			? [...new Set([...selected, key])]
			: selected.filter((value) => value !== key);
		clearPicker();
	}
	async function findCandidates(event?: SubmitEvent) {
		event?.preventDefault();
		if (!allowed || !selected.length || !sourceDigest || stale || pending) return;
		const ticket = candidateRequest.begin();
		candidateLoading = true;
		candidateError = '';
		candidates = null;
		teacher = null;
		returnRequest.abort();
		returnLessons = [];
		returnLessonKey = '';
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
		teacher = candidate;
		mode = 'substitution';
		taughtOfferingId = '';
		taughtActivity = '';
		returnDate = undefined;
		returnLessons = [];
		returnLessonKey = '';
		returnRequest.abort();
		returnError = '';
	}
	async function findReturnLessons() {
		if (!teacher || !sourceDigest || selected.length !== 1 || !returnDate || stale) return;
		const ticket = returnRequest.begin();
		returnLoading = true;
		returnError = '';
		returnLessons = [];
		returnLessonKey = '';
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
		if (!teacher || !allowed || stale || pending) return;
		const selectedTeacher = teacher;
		if (!taughtOfferingId && !taughtActivity.trim()) {
			candidateError = 'เลือกวิชาที่สอนจริง หรือระบุกิจกรรม';
			return;
		}
		if (mode === 'reciprocal' && (!returnLessonKey || selected.length !== 1)) {
			candidateError = 'เลือกคาบสอนคืนสำหรับนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน';
			return;
		}
		for (const key of selected)
			choiceLabels[key] = {
				teacherName: teacher.displayName,
				offeringName:
					teacher.taughtOfferings.find((value) => value.id === taughtOfferingId)?.name ??
					taughtActivity.trim()
			};
		assignments = [
			...assignments.filter((value) => !selected.includes(value.lessonKey)),
			...selected.map((lessonKey) => ({
				lessonKey,
				replacementTeacherId: selectedTeacher.teacherId,
				mode,
				taughtOfferingId: taughtOfferingId || null,
				taughtActivity: taughtActivity.trim() || null,
				returnLessonKey: mode === 'reciprocal' ? returnLessonKey : null
			}))
		];
		dirty = true;
		ondirty?.(true);
		teacher = null;
		candidates = null;
		selected = [];
		candidateError = '';
	}
	async function save() {
		if (!allowed || stale || !sourceDigest || pending) return;
		pending = true;
		error = '';
		const basis = { expectedRevision: baseRevision, sourceDigest, assignments };
		try {
			const result = await saveLeaveCoverage(detail.request.id, {
				requestId: intent.for(basis),
				...basis
			});
			if (!disposed) {
				dirty = false;
				ondirty?.(false);
				baseRevision = result.request.revision;
				sourceDigest = result.coverage.sourceDigest;
				assignments = coverageSelections(result.coverage);
				clearPicker();
				onsaved?.(result);
			}
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกแผนไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
	function acceptLatest() {
		baseRevision = detail.request.revision;
		sourceDigest = plan.sourceDigest;
		assignments = assignments.filter((value) =>
			lessons.some((lesson) => lesson.lessonKey === value.lessonKey)
		);
		clearPicker();
		error = '';
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
			description="ตรวจสอบคาบล่าสุดก่อนใช้แผนที่แก้ไข"
			actionLabel="ใช้ข้อมูลล่าสุดและเก็บแผนที่แก้ไข"
			onaction={acceptLatest}
		/>{/if}
	{#if error}<PageState
			variant="error"
			title="บันทึกแผนไม่สำเร็จ"
			description={error}
			actionLabel={onrefresh ? 'ตรวจสอบข้อมูลล่าสุด' : undefined}
			onaction={onrefresh}
		/>{/if}
	<div class="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(20rem,26rem)]">
		<div class="min-w-0 space-y-4">
			{#if allowed && lessons.length}<div class="flex flex-wrap items-center justify-between gap-2">
					<p class="text-sm">เลือก {selected.length} คาบ · ครูที่ค้นหาต้องว่างทุกคาบที่เลือก</p>
					<Button
						variant="outline"
						size="sm"
						disabled={pending || stale}
						onclick={() => {
							selected =
								selected.length === lessons.length ? [] : lessons.map((lesson) => lesson.lessonKey);
							clearPicker();
						}}>{selected.length === lessons.length ? 'ล้างการเลือก' : 'เลือกทุกคาบ'}</Button
					>
				</div>{/if}
			{#each plan.days as day (day.date)}
				{@const dayCounts = coverageCounts(day.lessons, assignments)}
				<section
					class="overflow-hidden rounded-xl border bg-card"
					aria-label={`คาบสอน ${leaveDate(day.date)}`}
				>
					<div
						class="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/20 px-4 py-3"
					>
						<h3 class="font-semibold">{leaveDate(day.date)}</h3>
						<p class="text-xs text-muted-foreground">
							เลือกครู {dayCounts.selected}/{dayCounts.total} · ตอบรับ {dayCounts.confirmed}/{dayCounts.total}
						</p>
					</div>
					<ul class="divide-y">
						{#each day.lessons as lesson (lesson.lessonKey)}
							{@const assignment = displayAssignment(lesson.lessonKey)}
							{@const labels = coverageLessonLabels(lesson)}
							<li class="space-y-3 p-4" data-testid="leave-coverage-lesson">
								<div class="flex items-start gap-3">
									{#if allowed}<Checkbox
											id={`coverage-${lesson.lessonKey}`}
											aria-label={`เลือก ${day.date} ${lesson.periodName}`}
											checked={selected.includes(lesson.lessonKey)}
											disabled={pending || stale || !sourceDigest}
											onCheckedChange={(value) => toggle(lesson.lessonKey, value)}
										/>{/if}
									<div class="min-w-0 flex-1">
										<p class="font-medium">
											{lesson.periodName} · {lesson.startTime.slice(0, 5)}–{lesson.endTime.slice(
												0,
												5
											)}
										</p>
										<p class="break-words">{labels.title}</p>
										{#if labels.details}<p class="text-sm text-muted-foreground">
												{labels.details}
											</p>{/if}
									</div>
								</div>
								{#if assignment}<div class="space-y-1 rounded-lg bg-muted/30 p-3 text-sm">
										<p class="font-medium">
											{lesson.assignment?.replacementTeacherId === assignment.replacementTeacherId
												? lesson.assignment.replacementTeacherName
												: (choiceLabels[lesson.lessonKey]?.teacherName ?? 'ครูที่เลือกใหม่')}
										</p>
										<p>
											{assignment.mode === 'reciprocal' ? 'แลกคาบสอน' : 'สอนแทน'} · {assignment.taughtActivity ||
												(lesson.assignment?.taughtOfferingId === assignment.taughtOfferingId
													? lesson.assignment?.taughtOfferingName
													: (choiceLabels[lesson.lessonKey]?.offeringName ?? 'วิชาที่เลือก'))}
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
												disabled={pending || stale}
												onclick={() => {
													assignments = assignments.filter(
														(value) => value.lessonKey !== lesson.lessonKey
													);
													dirty = true;
													ondirty?.(true);
												}}>นำครูออกจากคาบนี้</Button
											>{/if}
									</div>{:else}<p class="text-sm text-muted-foreground">ยังไม่ได้เลือกครู</p>{/if}
							</li>{/each}
					</ul>
				</section>{/each}
		</div>
		{#if allowed}<aside
				class="min-w-0 space-y-4 rounded-xl border bg-card p-4 xl:sticky xl:top-4"
				aria-label="ค้นหาครูและกำหนดคาบสอน"
			>
				<h3 class="font-semibold">เลือกครูสำหรับ {selected.length} คาบ</h3>
				<form onsubmit={findCandidates} class="space-y-3">
					<div class="space-y-2">
						<Label for="coverage-teacher-search">ชื่อครู</Label><Input
							id="coverage-teacher-search"
							bind:value={search}
							disabled={pending}
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
						disabled={pending || candidateLoading}
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
						disabled={pending || candidateLoading}
					/>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-subject"
							bind:checked={sameSubjectGroupOnly}
							disabled={pending || candidateLoading}
						/><Label for="coverage-same-subject">เฉพาะกลุ่มสาระเดียวกัน</Label>
					</div>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-students"
							bind:checked={sameStudentGroupOnly}
							disabled={pending || candidateLoading}
						/><Label for="coverage-same-students">สอนนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน</Label>
					</div>
					<div class="flex items-center gap-2">
						<Checkbox
							id="coverage-same-grade"
							bind:checked={sameGradeOnly}
							disabled={pending || candidateLoading}
						/><Label for="coverage-same-grade">สอนระดับชั้นเดียวกัน</Label>
					</div>

					<LoadingButton
						type="submit"
						loading={candidateLoading}
						disabled={!selected.length || !sourceDigest || stale || pending}
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
							label="วิธีจัดคาบ"
							value={mode}
							onchange={(value) => {
								mode = value === 'reciprocal' ? 'reciprocal' : 'substitution';
								taughtOfferingId = '';
								taughtActivity = '';
								returnRequest.abort();
								returnLessons = [];
								returnLessonKey = '';
							}}
							options={[
								{ value: 'substitution', label: 'สอนแทน' },
								...(selected.length === 1 ? [{ value: 'reciprocal', label: 'แลกคาบสอน' }] : [])
							]}
						/>
						{#if mode === 'substitution'}
							<Choice
								id="coverage-taught-offering"
								label="วิชาที่สอนจริง"
								bind:value={taughtOfferingId}
								onchange={() => {
									if (taughtOfferingId) taughtActivity = '';
								}}
								options={[
									{ value: '', label: 'ระบุกิจกรรมแทนวิชา' },
									...teacher.taughtOfferings.map((value) => ({
										value: value.id,
										label: value.name
									}))
								]}
							/>
							<div class="space-y-2">
								<Label for="coverage-activity">กิจกรรมที่สอนจริง / กิจกรรมเสริม (แทนวิชา)</Label
								><Textarea
									id="coverage-activity"
									bind:value={taughtActivity}
									maxlength={200}
									oninput={(event) => {
										if (event.currentTarget.value.trim()) taughtOfferingId = '';
									}}
									rows={2}
									placeholder="เช่น ทบทวนภาษาอังกฤษ หรือกิจกรรมอ่านหนังสือ"
								/>
							</div>
						{/if}
						{#if mode === 'reciprocal'}<div class="space-y-3 rounded-lg border p-3">
								<p class="text-sm">
									เลือกคาบคืนของนักเรียนกลุ่มเดียวกัน/ห้องเดียวกัน ระบบตรวจความว่างของครูทั้งสองฝ่าย
								</p>
								<div class="space-y-2">
									<Label for="coverage-return-date">วันที่สอนคืน</Label><DatePicker
										id="coverage-return-date"
										bind:value={returnDate}
										onValueChange={() => {
											returnRequest.abort();
											returnLessons = [];
											returnLessonKey = '';
										}}
									/>
								</div>
								<LoadingButton
									loading={returnLoading}
									disabled={!returnDate}
									onclick={findReturnLessons}>ค้นหาคาบสอนคืน</LoadingButton
								>{#if returnError}<PageState
										variant="error"
										title="ค้นหาคาบคืนไม่สำเร็จ"
										description={returnError}
									/>{/if}{#if returnLessons.length}<Choice
										id="coverage-return-lesson"
										label="คาบสอนคืน"
										bind:value={returnLessonKey}
										onchange={() => {
											const back = returnLessons.find(
												(value) => value.lessonKey === returnLessonKey
											);
											taughtOfferingId = back?.offeringId ?? '';
											taughtActivity = back?.offeringId
												? ''
												: (back?.offeringName ?? 'กิจกรรมตามตาราง');
										}}
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
						{#if mode === 'reciprocal' && returnLessonKey}<p
								class="rounded-lg bg-primary/5 p-3 text-sm"
							>
								วิชาที่สอนจริง: {returnLessons.find((value) => value.lessonKey === returnLessonKey)
									?.offeringName ?? 'กิจกรรมตามตาราง'} · อ้างอิงจากคาบที่แลก
							</p>{/if}
						<Button disabled={pending || stale} onclick={apply}
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
			<LoadingButton loading={pending} disabled={!dirty || stale || !sourceDigest} onclick={save}
				>บันทึกแผนคาบสอน</LoadingButton
			>
		</div>{/if}
</section>
