<script lang="ts">
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { authStore } from '#lib/stores/auth.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { registerAcademicContextDirtySource } from '#lib/academic-context/store.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		confirmYearRollover,
		getYearRolloverOptions,
		previewYearRollover,
		type YearRolloverOptions,
		type YearRolloverPreview,
		type YearRolloverOutcome,
		type YearRolloverPreviewInput,
		type ConfirmYearRolloverInput
	} from '#lib/api/academic-year-management.js';
	import {
		gradeLabel,
		rolloverOutcomeLabels,
		initialRolloverDraft,
		rosterDecisions,
		rosterDraftError,
		sourceRoomKey,
		type RolloverDraft
	} from '#lib/academic/lifecycle/year-management.js';
	import RolloverDecisionFields from './RolloverDecisionFields.svelte';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';

	import { Label } from '#lib/components/ui/label/index.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	let {
		yearId,
		yearName,
		disabled = false,
		onprepared
	}: {
		yearId: string;
		yearName: string;
		disabled?: boolean;
		onprepared: (outcome: YearRolloverOutcome, unassigned: number) => void;
	} = $props();
	let open = $state(false);
	let studentSearch = $state('');
	let chosenGroupKey = $state('');
	let editingStudents = $state<string[]>([]);
	let studentPage = $state(0);
	const pageSize = 25;
	let options = $state<YearRolloverOptions | null>(null);
	let targetYearId = $state('');
	let outcomeEffectiveOn = $state<string | undefined>();
	let targetPlacementStartsOn = $state<string | undefined>();
	let preview = $state.raw<YearRolloverPreview | null>(null);
	let drafts = $state<Record<string, RolloverDraft>>({});
	let bulkDrafts = $state<Record<string, RolloverDraft>>({});
	let loading = $state(false);
	let busy = $state(false);
	let error = $state('');
	let stale = $state(false);
	let previewKey = $state('');
	let pending = $state<ConfirmYearRolloverInput | null>(null);
	const latest = new LatestRequest();
	let alive = false;
	const canRead = $derived($can.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
	const canConfirm = $derived(canRead && $can.has(PERMISSIONS.ACADEMIC_PROMOTION_EXECUTE_SCHOOL));
	const matchingStudents = $derived(
		(preview?.students ?? []).filter(
			(student) =>
				!studentSearch.trim() ||
				`${student.studentName} ${student.studentCode ?? ''}`
					.toLocaleLowerCase('th')
					.includes(studentSearch.trim().toLocaleLowerCase('th'))
		)
	);
	const visibleStudents = $derived(
		matchingStudents.slice(studentPage * pageSize, (studentPage + 1) * pageSize)
	);
	const decisions = $derived(preview ? rosterDecisions(preview.students, drafts) : []);
	const input = $derived<YearRolloverPreviewInput | null>(
		targetYearId && outcomeEffectiveOn && targetPlacementStartsOn
			? {
					sourceYearId: yearId,
					targetYearId,
					outcomeEffectiveOn,
					targetPlacementStartsOn,
					decisions
				}
			: null
	);
	const unassignedCount = $derived(
		preview
			? preview.students.filter((student) => {
					const draft = drafts[student.studentAcademicYearId];
					return (
						draft &&
						(draft.outcome === 'promote' || draft.outcome === 'repeat') &&
						!draft.targetHomeroomId
					);
				}).length
			: 0
	);
	const draftError = $derived(preview ? rosterDraftError(preview, drafts) : '');
	const changed = $derived(!input || JSON.stringify(input) !== previewKey);
	const groups = $derived.by(() => {
		const result = new SvelteMap<string, NonNullable<typeof preview>['students']>();
		for (const student of preview?.students ?? []) {
			const key = sourceRoomKey(student);
			const members = result.get(key);
			if (members) members.push(student);
			else result.set(key, [student]);
		}
		return [...result.entries()];
	});

	const selectedGroup = $derived(groups.find(([key]) => key === chosenGroupKey) ?? groups[0]);
	function decisionSummary(draft: RolloverDraft) {
		if (draft.retainedDecision)
			return `${draft.retainedDecision.outcome === 'conditional' ? 'มีเงื่อนไข (ประวัติเดิม)' : rolloverOutcomeLabels[draft.retainedDecision.outcome]} · ยืนยันปลายทางเดิมแล้ว`;
		if (!draft.outcome) return 'ยังไม่เลือกผล';
		return [
			rolloverOutcomeLabels[draft.outcome],
			...(draft.outcome === 'promote' || draft.outcome === 'repeat'
				? [
						gradeLabel(preview?.grades.find((grade) => grade.id === draft.targetGradeLevelId)),
						preview?.homerooms.find((room) => room.id === draft.targetHomeroomId)?.name ??
							'ยังไม่จัดห้อง'
					]
				: [])
		].join(' · ');
	}
	async function loadOptions() {
		if (!canRead || busy) return;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		loading = true;
		error = '';
		try {
			const result = await getYearRolloverOptions({ signal });
			if (!latest.isCurrent(revision) || identity !== appIdentityKey()) return;
			options = result;
		} catch (cause) {
			if (!isAbortError(cause) && latest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดปีปลายทางไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	function changeOpen(value: boolean) {
		if (busy) return;
		open = value;
		if (value) {
			if (!options) void loadOptions();
		} else {
			latest.abort();
			loading = false;
		}
	}
	function changeTarget(value: string) {
		targetYearId = value;
		preview = null;
		drafts = {};
		bulkDrafts = {};
		pending = null;
		previewKey = '';
		stale = false;
	}
	async function loadPreview() {
		if (!input || !canRead || busy) return;
		const request = structuredClone(input);
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		loading = true;
		error = '';
		try {
			const result = await previewYearRollover(request, { signal });
			if (!latest.isCurrent(revision) || identity !== appIdentityKey()) return;
			const nextDrafts: Record<string, RolloverDraft> = {};
			for (const student of result.students)
				nextDrafts[student.studentAcademicYearId] =
					drafts[student.studentAcademicYearId] ?? initialRolloverDraft(student);
			drafts = nextDrafts;
			preview = result;
			studentPage = Math.min(
				studentPage,
				Math.max(0, Math.ceil(result.students.length / pageSize) - 1)
			);
			// Suggested rows become explicit declarations on the next preview, before confirmation.
			previewKey = JSON.stringify(request);
			stale = false;
			pending = null;
		} catch (cause) {
			if (!isAbortError(cause) && latest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดตัวอย่างไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	function applyBulk(key: string) {
		const draft = bulkDrafts[key];
		if (!draft || pending || busy) return;
		const next = { ...drafts };
		for (const student of preview?.students ?? [])
			if (
				sourceRoomKey(student) === key &&
				!drafts[student.studentAcademicYearId]?.retainedDecision
			)
				next[student.studentAcademicYearId] = { ...draft };
		drafts = next;
		editingStudents = editingStudents.filter(
			(studentId) =>
				!preview?.students.some(
					(student) => student.studentAcademicYearId === studentId && sourceRoomKey(student) === key
				)
		);
	}
	async function confirm() {
		if (
			!canConfirm ||
			!preview ||
			!input ||
			changed ||
			stale ||
			draftError ||
			!preview.canConfirm ||
			busy
		)
			return;
		pending ??= {
			requestId: crypto.randomUUID(),
			sourceChecksum: preview.sourceChecksum,
			preview: structuredClone(input)
		};
		const request = pending;
		const identity = appIdentityKey();
		busy = true;
		error = '';
		try {
			const outcome = await confirmYearRollover(request);
			if (!alive || identity !== appIdentityKey()) return;
			const unassigned = unassignedCount;
			open = false;
			pending = null;
			onprepared(outcome, unassigned);
			preview = null;
			drafts = {};
			bulkDrafts = {};
		} catch (cause) {
			if (!alive || identity !== appIdentityKey()) return;
			error = cause instanceof Error ? cause.message : 'ยังยืนยันผลการบันทึกไม่ได้';
			if (cause instanceof ApiClientError && cause.status === 409) {
				stale = true;
				pending = null;
			}
		} finally {
			busy = false;
		}
	}
	onMount(() => {
		alive = true;
		let identity = appIdentityKey();
		const unsubscribeIdentity = authStore.subscribe(() => {
			const next = appIdentityKey();
			if (identity && identity !== next) {
				latest.abort();
				open = false;
				preview = null;
				drafts = {};
				bulkDrafts = {};
				options = null;
				pending = null;
				targetYearId = '';
				outcomeEffectiveOn = undefined;
				targetPlacementStartsOn = undefined;
			}
			identity = next;
		});
		const unregister = registerAcademicContextDirtySource(
			`year-rollover:${yearId}`,
			() => open || busy || Object.keys(drafts).length > 0
		);
		const unsubscribe = can.subscribe((permissions) => {
			if (!permissions.has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)) {
				latest.abort();
				open = false;
				preview = null;
				drafts = {};
				options = null;
				pending = null;
			}
		});
		return () => {
			alive = false;
			latest.abort();
			unregister();
			unsubscribe();
			unsubscribeIdentity();
		};
	});
</script>

{#if canRead}<Button {disabled} onclick={() => changeOpen(true)}>ขึ้นปีการศึกษาใหม่</Button>{/if}
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex max-h-[90dvh] flex-col overflow-hidden sm:max-w-6xl"
		showCloseButton={!busy}
		onInteractOutside={(event) => {
			if (busy) event.preventDefault();
		}}
		onEscapeKeydown={(event) => {
			if (busy) event.preventDefault();
		}}
	>
		<Dialog.Header
			><Dialog.Title>เตรียมรายชื่อนักเรียนปีใหม่ (ยังไม่เริ่มใช้)</Dialog.Title><Dialog.Description
				>จาก {yearName} เลือกห้องปลายทางและข้อยกเว้นก่อนยืนยันครั้งเดียว เก็บตัวตนและประวัติปีเดิมไว้
				การเริ่มใช้ปีใหม่ทำแยกกัน</Dialog.Description
			></Dialog.Header
		>
		<div class="min-h-0 flex-1 space-y-5 overflow-y-auto pe-1">
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
			{#if !options && loading}<PageSkeleton variant="cards" rows={2} />{:else if !options}<Button
					variant="outline"
					onclick={() => void loadOptions()}>ลองโหลดปีปลายทางอีกครั้ง</Button
				>{:else}
				<div class="grid gap-4 md:grid-cols-3">
					<div class="space-y-2">
						<Label for="rollover-target">ปีปลายทาง</Label><Select.Root
							type="single"
							value={targetYearId}
							disabled={busy || !!pending}
							onValueChange={changeTarget}
							><Select.Trigger id="rollover-target" class="w-full"
								><span
									>{options.targetYears.find((year) => year.academicYearId === targetYearId)
										?.name ?? 'เลือกปีที่กำลังเตรียม'}</span
								></Select.Trigger
							><Select.Content
								>{#each options.targetYears.filter((year) => year.academicYearId !== yearId) as year (year.academicYearId)}<Select.Item
										value={year.academicYearId}>{year.name}</Select.Item
									>{/each}</Select.Content
							></Select.Root
						>{#if options.targetYears.length === 0}<p class="text-sm text-muted-foreground">
								สร้างปีปลายทางในส่วนตั้งค่าปีและภาคเรียนก่อน
							</p>{/if}
					</div>
					<div class="space-y-2">
						<Label for="rollover-effective">วันสิ้นสุดสถานะปีต้นทาง</Label><DatePicker
							id="rollover-effective"
							ariaLabel="วันสิ้นสุดสถานะปีต้นทาง"
							bind:value={outcomeEffectiveOn}
							required
							disabled={busy || !!pending}
						/>
					</div>
					<div class="space-y-2">
						<Label for="rollover-start">วันเริ่มจัดห้องปีใหม่</Label><DatePicker
							id="rollover-start"
							ariaLabel="วันเริ่มจัดห้องปีใหม่"
							bind:value={targetPlacementStartsOn}
							required
							disabled={busy || !!pending}
						/>
					</div>
				</div>
				<p class="text-sm text-muted-foreground">
					วันเริ่มจัดห้องปีใหม่ต้องอยู่หลังวันสิ้นสุดปีต้นทาง ป.6 และ ม.3
					เลือกเรียนต่อในโรงเรียนเดิมได้ โดยระบุผลและชั้นปลายทางอย่างชัดเจน
				</p>

				{#if preview}
					<div class="space-y-3 rounded-xl border p-4">
						<h3 class="font-semibold">
							{preview.sourceYear.name} → {preview.targetYear.name} · {preview.students.length} คน
						</h3>
						{#each preview.findings as finding (finding.code)}<p
								class={finding.severity === 'blocking'
									? 'text-sm text-destructive'
									: 'text-sm text-muted-foreground'}
							>
								{finding.message}
							</p>{/each}
						{#if preview.sourceYear.status !== 'closed'}<p class="text-sm">
								ปิดภาคเรียนและปีต้นทางที่หน้าจัดการปีการศึกษาก่อนยืนยันรายชื่อ
								สามารถปิดหน้าต่างนี้แล้วกลับมาตรวจตัวอย่างล่าสุด โดยตัวเลือกที่แก้ไว้ยังอยู่
							</p>{/if}
					</div>
					{#if selectedGroup}{@const [key, students] = selectedGroup}{@const first = students[0]}
						<section
							class="space-y-3 rounded-xl border border-primary/20 bg-primary/5 p-4"
							aria-label="กำหนดปลายทางทีละห้อง"
						>
							<h3 class="font-semibold">กำหนดปลายทางทีละห้อง</h3>
							<p class="text-sm text-muted-foreground">
								ใช้กับนักเรียนทั้งห้องก่อน แล้วแก้เฉพาะรายที่ต่างออกไปด้านล่าง
							</p>
							<div class="max-w-sm space-y-2">
								<Label for="bulk-source-room">ห้องต้นทาง</Label><Select.Root
									type="single"
									value={key}
									onValueChange={(value) => {
										chosenGroupKey = value;
									}}
									><Select.Trigger id="bulk-source-room" class="w-full"
										>{first.sourceHomeroomName ??
											gradeLabel(
												preview.grades.find((grade) => grade.id === first.sourceGradeLevelId)
											)} · {students.length} คน</Select.Trigger
									><Select.Content
										>{#each groups as [groupKey, members] (groupKey)}<Select.Item value={groupKey}
												>{members[0].sourceHomeroomName ??
													gradeLabel(
														preview.grades.find(
															(grade) => grade.id === members[0].sourceGradeLevelId
														)
													)} · {members.length} คน</Select.Item
											>{/each}</Select.Content
									></Select.Root
								>
							</div>
							<RolloverDecisionFields
								id={`bulk-${first.studentAcademicYearId}`}
								draft={bulkDrafts[key] ?? initialRolloverDraft(first)}
								student={first}
								{preview}
								disabled={busy || !!pending}
								onchange={(draft) => {
									bulkDrafts = { ...bulkDrafts, [key]: draft };
								}}
							/>
							<Button
								size="sm"
								variant="outline"
								disabled={busy || !!pending || !bulkDrafts[key]?.outcome}
								onclick={() => applyBulk(key)}>ใช้กับนักเรียนในห้องนี้</Button
							>
						</section>{/if}
					<div class="max-w-md space-y-2">
						<Label for="roster-search">ค้นหานักเรียน</Label><Input
							id="roster-search"
							value={studentSearch}
							oninput={(event) => {
								studentSearch = event.currentTarget.value;
								studentPage = 0;
							}}
						/>
					</div>
					<div class="space-y-2" aria-label="รายชื่อนักเรียนขึ้นปีใหม่">
						{#each visibleStudents as student (student.studentAcademicYearId)}
							{@const draft = drafts[student.studentAcademicYearId]}
							{@const studentError = rosterDraftError(
								{ ...preview, students: [student] },
								{ [student.studentAcademicYearId]: draft }
							)}
							{@const editing =
								!!studentError || editingStudents.includes(student.studentAcademicYearId)}
							<section class="space-y-3 rounded-lg border bg-card p-3">
								<div class="flex flex-wrap items-start justify-between gap-2">
									<div>
										<h3 class="font-medium">
											{student.studentName}
											<span class="text-sm font-normal text-muted-foreground"
												>{student.studentCode ?? ''}</span
											>
										</h3>
										<p class="text-sm text-muted-foreground">
											{student.sourceHomeroomName ??
												gradeLabel(
													preview.grades.find((grade) => grade.id === student.sourceGradeLevelId)
												)} → {decisionSummary(draft)}
										</p>
										{#if !student.suggestedDecision && studentError}<p
												class="mt-1 text-sm text-primary"
											>
												เลือกผลอย่างชัดเจนสำหรับนักเรียนคนนี้
											</p>{/if}
									</div>
									{#if !editing && !draft.retainedDecision}<Button
											size="sm"
											variant="ghost"
											disabled={busy || !!pending}
											onclick={() => {
												editingStudents = [...editingStudents, student.studentAcademicYearId];
											}}>แก้ไข</Button
										>{/if}
								</div>
								{#if draft.retainedDecision}<p class="text-sm text-muted-foreground">
										รักษาปลายทางและหลักฐานเดิม{draft.retainedDecision.condition
											? ` · เงื่อนไข: ${draft.retainedDecision.condition}`
											: ''}
									</p>{:else if editing}<RolloverDecisionFields
										id={student.studentAcademicYearId}
										{draft}
										{student}
										{preview}
										disabled={busy || !!pending}
										onchange={(next) => {
											drafts = { ...drafts, [student.studentAcademicYearId]: next };
											if (!editingStudents.includes(student.studentAcademicYearId))
												editingStudents = [...editingStudents, student.studentAcademicYearId];
										}}
									/>{#if !studentError}<Button
											size="sm"
											variant="outline"
											onclick={() => {
												editingStudents = editingStudents.filter(
													(id) => id !== student.studentAcademicYearId
												);
											}}>เสร็จแล้ว</Button
										>{/if}{/if}
							</section>{/each}
					</div>
					<div class="flex flex-wrap items-center gap-3">
						<Button
							variant="outline"
							size="sm"
							disabled={studentPage === 0}
							onclick={() => {
								studentPage -= 1;
							}}>ก่อนหน้า</Button
						><span class="text-sm"
							>{matchingStudents.length} คน · หน้า {studentPage + 1} / {Math.max(
								1,
								Math.ceil(matchingStudents.length / pageSize)
							)}</span
						><Button
							variant="outline"
							size="sm"
							disabled={(studentPage + 1) * pageSize >= matchingStudents.length}
							onclick={() => {
								studentPage += 1;
							}}>ถัดไป</Button
						>
					</div>
					<p class="text-sm">
						ยังไม่จัดห้อง {unassignedCount} คน — เตรียมรายชื่อได้ แต่ยังไม่พร้อมใช้เช็กชื่อในห้องเรียน
					</p>
					{#if $can.has(PERMISSIONS.HOMEROOM_MANAGE_SCHOOL)}<Button
							variant="outline"
							href={resolve(`staff/academic/homerooms?academicYearId=${targetYearId}`)}
							>จัดห้องปีปลายทาง</Button
						>{/if}
					{#if draftError}<p class="text-sm text-destructive">{draftError}</p>{:else if changed}<p
							class="text-sm text-muted-foreground"
						>
							ตัวเลือกเปลี่ยนแล้ว ตรวจตัวอย่างล่าสุดก่อนยืนยัน
						</p>{/if}
					{#if pending && error && !stale}<p class="text-sm">
							ยังยืนยันผลจากเซิร์ฟเวอร์ไม่ได้ กดลองยืนยันอีกครั้งเพื่อใช้คำขอเดิม
							โดยไม่สร้างรายการซ้ำ
						</p>{/if}
					{#if stale}<p class="text-sm text-destructive">
							ข้อมูลเปลี่ยนแล้ว กรุณาตรวจตัวอย่างล่าสุด ตัวเลือกที่แก้ไว้ยังอยู่
						</p>{/if}
				{/if}
			{/if}
		</div>
		<Dialog.Footer class="border-t pt-4"
			><Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}
				>กลับไปหน้าจัดการ</Button
			><LoadingButton
				variant="outline"
				{loading}
				disabled={!input || busy || !!pending}
				onclick={() => void loadPreview()}
				>{preview ? 'ตรวจตัวอย่างล่าสุด' : 'ดูตัวอย่างรายชื่อนักเรียน'}</LoadingButton
			><LoadingButton
				loading={busy}
				disabled={!canConfirm ||
					!preview?.canConfirm ||
					changed ||
					stale ||
					!!draftError ||
					loading}
				onclick={() => void confirm()}
				>{pending ? 'ลองยืนยันอีกครั้ง' : 'ยืนยันเตรียมรายชื่อนักเรียน'}</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
