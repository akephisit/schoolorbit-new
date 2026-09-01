<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		getAssessmentPlan,
		listAssessmentPhaseControls,
		listAssessmentPlans,
		saveAssessmentPlan,
		updateAssessmentPhaseControl,
		type AssessmentExamArrangement,
		type AssessmentPhase,
		type AssessmentPhaseCode,
		type AssessmentPhaseControl,
		type AssessmentPlanDetail,
		type AssessmentPlanSummary,
		type AssessmentReadinessFinding,
		type SaveAssessmentPhaseRequest
	} from '$lib/api/academicAssessments';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import { Switch } from '$lib/components/ui/switch';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import {
		BookOpenCheck,
		CalendarClock,
		Check,
		ChevronRight,
		CircleAlert,
		Clock3,
		CloudAlert,
		CloudCheck,
		Loader2,
		Search,
		ShieldCheck,
		Sparkles,
		UserRoundCheck,
		UsersRound
	} from 'lucide-svelte';

	type ReadyFilter = 'all' | 'ready' | 'attention';
	type ExamFilter = 'all' | AssessmentExamArrangement;
	type SaveState = 'idle' | 'pending' | 'saving' | 'saved' | 'error';

	const phaseCodes: AssessmentPhaseCode[] = ['before_midterm', 'midterm', 'after_midterm', 'final'];
	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const currentUserId = $derived($authStore.user?.id ?? '');

	let plans = $state.raw<AssessmentPlanSummary[]>([]);
	let phaseControls = $state.raw<AssessmentPhaseControl[]>([]);
	let detail = $state.raw<AssessmentPlanDetail | null>(null);
	let draftPhases = $state<SaveAssessmentPhaseRequest[]>([]);
	let draftCoordinatorId = $state('');
	let searchQuery = $state('');
	let readyFilter = $state<ReadyFilter>('all');
	let examFilter = $state<ExamFilter>('all');
	let sheetOpen = $state(false);
	let loading = $state(false);
	let detailLoading = $state(false);
	let saveState = $state<SaveState>('idle');
	let dirty = $state(false);
	let saving = $state(false);
	let controlBusyId = $state('');
	let errorMessage = $state('');
	let lastSavedAt = $state<Date | null>(null);
	let revision = 0;
	let draftRevision = 0;
	let saveTimer: ReturnType<typeof setTimeout> | undefined;

	const canRead = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_READ_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL
		)
	);
	const canManageSchool = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL
		)
	);
	const canManageAssigned = $derived($can.has(PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED));
	const canEditDetail = $derived.by(() => {
		if (!detail) return false;
		if (canManageSchool) return true;
		if (!canManageAssigned || !currentUserId) return false;
		if (detail.assessmentCoordinatorId === currentUserId) return true;
		return (
			!detail.id &&
			detail.coordinatorCandidates.some((candidate) => candidate.teacherId === currentUserId)
		);
	});
	const filteredPlans = $derived.by(() => {
		const query = searchQuery.trim().toLocaleLowerCase('th');
		return plans.filter((plan) => {
			if (readyFilter === 'ready' && !plan.readiness.ready) return false;
			if (readyFilter === 'attention' && plan.readiness.ready) return false;
			if (
				examFilter !== 'all' &&
				!plan.phases.some((phase) => phase.examArrangement === examFilter)
			)
				return false;
			if (!query) return true;
			return [
				plan.offeringCode,
				plan.offeringName,
				plan.subjectVersionDisplayLabel,
				plan.assessmentCoordinatorName ?? ''
			].some((value) => value.toLocaleLowerCase('th').includes(query));
		});
	});
	const readyCount = $derived(plans.filter((plan) => plan.readiness.ready).length);
	const attentionCount = $derived(plans.length - readyCount);
	const draftTotal = $derived(
		draftPhases.reduce((total, phase) => total + (Number(phase.maxScore) || 0), 0)
	);

	const readyOptions: Array<{ value: ReadyFilter; label: string }> = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'ready', label: 'พร้อมใช้งาน' },
		{ value: 'attention', label: 'ต้องตรวจสอบ' }
	];
	const examOptions: Array<{ value: ExamFilter; label: string }> = [
		{ value: 'all', label: 'การสอบทุกแบบ' },
		{ value: 'in_timetable', label: 'สอบในตาราง' },
		{ value: 'outside_timetable', label: 'สอบนอกตาราง' },
		{ value: 'none', label: 'ไม่จัดสอบ' }
	];
	const arrangementOptions: Array<{ value: AssessmentExamArrangement; label: string }> = [
		{ value: 'none', label: 'ไม่จัดสอบ' },
		{ value: 'in_timetable', label: 'สอบในตารางสอบ' },
		{ value: 'outside_timetable', label: 'สอบนอกตารางสอบ' }
	];
	const noCourseOfferingPrerequisite: AcademicPrerequisite = {
		key: 'assessment-course-offering',
		status: 'missing',
		title: 'สร้างรายการเปิดสอนก่อนกำหนดโครงสร้างคะแนน',
		description:
			'หน้านี้ใช้รายวิชาที่เปิดสอนในภาคเรียน ไม่ได้ดึงรายวิชาทุกตัวจากทะเบียนหลักสูตรโดยตรง',
		actionLabel: 'ไปจัดรายการเปิดสอน',
		href: '/staff/academic/delivery'
	};

	function phaseFor(
		plan: AssessmentPlanSummary,
		code: AssessmentPhaseCode
	): AssessmentPhase | null {
		return plan.phases.find((phase) => phase.phaseCode === code) ?? null;
	}

	function phaseLabel(code: AssessmentPhaseCode): string {
		return (
			{
				before_midterm: 'ก่อนกลางภาค',
				midterm: 'กลางภาค',
				after_midterm: 'หลังกลางภาค',
				final: 'ปลายภาค'
			} satisfies Record<AssessmentPhaseCode, string>
		)[code];
	}

	function arrangementLabel(arrangement: AssessmentExamArrangement): string {
		return (
			{
				none: 'ไม่จัดสอบ',
				in_timetable: 'ในตาราง',
				outside_timetable: 'นอกตาราง'
			} satisfies Record<AssessmentExamArrangement, string>
		)[arrangement];
	}

	function findingLabel(finding: AssessmentReadinessFinding): string {
		return (
			{
				missing_coordinator: 'ยังไม่ได้กำหนดผู้รับผิดชอบโครงสร้างคะแนน',
				coordinator_not_candidate: 'ผู้รับผิดชอบไม่ได้สอนรายวิชานี้แล้ว',
				missing_phase: 'ช่วงคะแนนมาตรฐานยังไม่ครบ 4 ช่วง',
				total_mismatch: 'คะแนนรวมยังไม่ตรงกับเกณฑ์ของรายวิชา',
				midterm_missing_exam_duration: 'การสอบกลางภาคยังไม่ระบุเวลา',
				final_missing_exam_duration: 'การสอบปลายภาคยังไม่ระบุเวลา'
			} satisfies Record<AssessmentReadinessFinding, string>
		)[finding];
	}

	function clonePhases(source: AssessmentPlanDetail): SaveAssessmentPhaseRequest[] {
		return source.phases
			.toSorted((left, right) => left.order - right.order)
			.map((phase) => ({
				id: phase.id ?? null,
				phaseCode: phase.phaseCode,
				maxScore: phase.maxScore,
				examArrangement: phase.examArrangement,
				examDurationMinutes: phase.examDurationMinutes ?? null
			}));
	}

	function localDraftError(): string | null {
		if (draftPhases.length !== 4) return 'ช่วงคะแนนต้องมีครบ 4 ช่วง';
		for (const phase of draftPhases) {
			if (!/^\d+(\.\d{1,2})?$/.test(phase.maxScore) || Number(phase.maxScore) < 0) {
				return `คะแนนช่วง${phaseLabel(phase.phaseCode)}ต้องเป็นเลขตั้งแต่ 0 และมีทศนิยมไม่เกิน 2 ตำแหน่ง`;
			}
			if (
				phase.examArrangement !== 'none' &&
				phase.examDurationMinutes != null &&
				phase.examDurationMinutes <= 0
			) {
				return `เวลาสอบช่วง${phaseLabel(phase.phaseCode)}ต้องมากกว่า 0 นาที`;
			}
		}
		return null;
	}

	function markDirty(): void {
		dirty = true;
		draftRevision += 1;
		saveState = 'pending';
		if (saveTimer) clearTimeout(saveTimer);
		saveTimer = setTimeout(() => void persistDraft(), 750);
	}

	function flushAutosave(): void {
		if (saveTimer) clearTimeout(saveTimer);
		saveTimer = undefined;
		void persistDraft();
	}

	async function loadWorkspace(termId: string): Promise<void> {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const rows = await listAssessmentPlans({ academicTermId: termId });
			if (current !== revision) return;
			const controls = await listAssessmentPhaseControls(termId);
			if (current !== revision) return;
			plans = rows;
			phaseControls = controls;
			detail = null;
			draftPhases = [];
			draftCoordinatorId = '';
			dirty = false;
			saveState = 'idle';
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดโครงสร้างคะแนนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function refreshPlans(): Promise<void> {
		if (!academicTermId) return;
		plans = await listAssessmentPlans({ academicTermId });
	}

	async function openPlan(plan: AssessmentPlanSummary): Promise<void> {
		if (saving) {
			toast.info('กำลังบันทึกรายการเดิม กรุณารอสักครู่');
			return;
		}
		if (dirty) {
			await persistDraft();
			if (dirty) {
				toast.warning('แก้ข้อมูลที่แจ้งเตือนให้เรียบร้อยก่อนเปิดรายวิชาอื่น');
				return;
			}
		}
		sheetOpen = true;
		detailLoading = true;
		errorMessage = '';
		try {
			const loaded = await getAssessmentPlan(plan.offeringId);
			detail = loaded;
			draftPhases = clonePhases(loaded);
			draftCoordinatorId =
				loaded.assessmentCoordinatorId ??
				(canManageSchool
					? (loaded.suggestedCoordinatorId ?? '')
					: loaded.coordinatorCandidates.some((candidate) => candidate.teacherId === currentUserId)
						? currentUserId
						: '');
			dirty = false;
			saveState = 'idle';
			lastSavedAt = null;
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดรายละเอียดโครงสร้างคะแนนไม่สำเร็จ';
		} finally {
			detailLoading = false;
		}
	}

	async function persistDraft(): Promise<void> {
		if (saveTimer) clearTimeout(saveTimer);
		saveTimer = undefined;
		if (!detail || !dirty || !canEditDetail || saving) return;
		const validationError = localDraftError();
		if (validationError) {
			errorMessage = validationError;
			saveState = 'error';
			return;
		}

		const offeringId = detail.offeringId;
		const revisionAtStart = draftRevision;
		const payloadPhases = draftPhases.map((phase) => ({
			...phase,
			examDurationMinutes: phase.examArrangement === 'none' ? null : phase.examDurationMinutes
		}));
		saving = true;
		saveState = 'saving';
		errorMessage = '';
		try {
			const saved = await saveAssessmentPlan(offeringId, {
				rowVersion: detail.rowVersion ?? null,
				assessmentCoordinatorId: draftCoordinatorId || null,
				phases: payloadPhases
			});
			if (detail?.offeringId !== offeringId) return;
			detail = saved;
			lastSavedAt = new Date();
			if (draftRevision === revisionAtStart) {
				draftPhases = clonePhases(saved);
				draftCoordinatorId = saved.assessmentCoordinatorId ?? '';
				dirty = false;
				saveState = 'saved';
			} else {
				saveState = 'pending';
			}
			try {
				await refreshPlans();
			} catch {
				toast.warning('บันทึกแล้ว แต่ยังรีเฟรชตารางภาพรวมไม่ได้');
			}
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกโครงสร้างคะแนนไม่สำเร็จ';
			saveState = 'error';
			toast.error(errorMessage);
		} finally {
			saving = false;
			if (dirty && draftRevision !== revisionAtStart && saveState !== 'error') markDirty();
		}
	}

	function applySuggestedCoordinator(): void {
		if (!detail?.suggestedCoordinatorId) return;
		draftCoordinatorId = detail.suggestedCoordinatorId;
		markDirty();
	}

	function updateArrangement(
		phase: SaveAssessmentPhaseRequest,
		arrangement: AssessmentExamArrangement
	): void {
		phase.examArrangement = arrangement;
		if (arrangement === 'none') phase.examDurationMinutes = null;
		markDirty();
	}

	async function togglePhaseControl(
		control: AssessmentPhaseControl,
		field: 'itemEditingEnabled' | 'scoreEntryEnabled'
	): Promise<void> {
		if (!canManageSchool || controlBusyId) return;
		controlBusyId = control.id;
		try {
			const saved = await updateAssessmentPhaseControl(control.id, {
				rowVersion: control.rowVersion,
				itemEditingEnabled:
					field === 'itemEditingEnabled' ? !control.itemEditingEnabled : control.itemEditingEnabled,
				scoreEntryEnabled:
					field === 'scoreEntryEnabled' ? !control.scoreEntryEnabled : control.scoreEntryEnabled
			});
			phaseControls = phaseControls.map((item) => (item.id === saved.id ? saved : item));
			toast.success(`บันทึกสิทธิ์ช่วง${saved.label}แล้ว`);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกการเปิดกรอกคะแนนไม่สำเร็จ');
		} finally {
			controlBusyId = '';
		}
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		const unregisterDirty = registerAcademicContextDirtySource(
			'academic-assessment-plan',
			() => dirty || saving
		);
		const unsubscribe = academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadWorkspace(termId);
			}
		});
		return () => {
			if (saveTimer) clearTimeout(saveTimer);
			unsubscribe();
			unregisterDirty();
		};
	});
</script>

<PageShell
	title="โครงสร้างคะแนนรายวิชา"
	description="ตรวจคะแนนเต็ม ผู้รับผิดชอบ และรูปแบบการสอบของ 4 ช่วงมาตรฐานในภาคเรียนเดียวกัน"
>
	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูโครงสร้างคะแนน"
			description="ติดต่อผู้ดูแลเพื่อขอสิทธิ์อ่านโครงสร้างคะแนนหรือรายการเปิดสอน"
		/>
	{:else if !academicTermId}
		<PageState
			variant="empty"
			title="เลือกภาคเรียนก่อน"
			description="ใช้ตัวเลือกปีการศึกษาและภาคเรียนบนแถบด้านบน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={8} />
	{:else if errorMessage && plans.length === 0}
		<PageState
			variant="error"
			title="โหลดโครงสร้างคะแนนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicTermId)}
		/>
	{:else}
		<div class="space-y-5">
			<section class="overflow-hidden rounded-xl border bg-card shadow-sm">
				<div class="flex flex-wrap items-center justify-between gap-4 border-b px-5 py-4">
					<div>
						<div class="flex items-center gap-2">
							<CalendarClock class="size-5 text-primary" />
							<h2 class="font-semibold">ช่วงการทำงานของครู</h2>
						</div>
						<p class="mt-1 text-sm text-muted-foreground">
							เปิดแยกได้ระหว่างการสร้างรายการคะแนนย่อยกับการกรอกคะแนนนักเรียน
						</p>
					</div>
					{#if !canManageSchool}<Badge variant="outline">ดูสถานะเท่านั้น</Badge>{/if}
				</div>
				<div class="grid divide-y sm:grid-cols-2 sm:divide-x sm:divide-y-0 xl:grid-cols-4">
					{#each phaseControls as control (control.id)}
						<div class="space-y-3 px-5 py-4">
							<div class="flex items-center justify-between gap-3">
								<p class="font-medium">{control.label}</p>
								<span class="font-mono text-xs text-muted-foreground">0{control.order}</span>
							</div>
							<div class="flex items-center justify-between gap-3 text-sm">
								<Label for={`item-control-${control.id}`}>แก้รายการคะแนน</Label>
								<Switch
									id={`item-control-${control.id}`}
									checked={control.itemEditingEnabled}
									disabled={!canManageSchool || Boolean(controlBusyId)}
									onclick={() => togglePhaseControl(control, 'itemEditingEnabled')}
								/>
							</div>
							<div class="flex items-center justify-between gap-3 text-sm">
								<Label for={`score-control-${control.id}`}>กรอกคะแนนนักเรียน</Label>
								<Switch
									id={`score-control-${control.id}`}
									checked={control.scoreEntryEnabled}
									disabled={!canManageSchool || Boolean(controlBusyId)}
									onclick={() => togglePhaseControl(control, 'scoreEntryEnabled')}
								/>
							</div>
						</div>
					{/each}
				</div>
			</section>

			{#if plans.length === 0}
				<AcademicPrerequisiteNotice prerequisite={noCourseOfferingPrerequisite} />
			{:else}
				<Card.Root class="gap-0 py-0">
					<Card.Header class="border-b py-5">
						<div class="flex flex-wrap items-start justify-between gap-4">
							<div>
								<Card.Title>ภาพรวมรายวิชา</Card.Title>
								<Card.Description class="mt-1">
									{plans.length} รายวิชา · พร้อมใช้ {readyCount} · ต้องตรวจ {attentionCount}
								</Card.Description>
							</div>
							<div class="flex flex-wrap items-center gap-2">
								<div class="relative min-w-64 flex-1">
									<Search
										class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
									/>
									<Label class="sr-only" for="assessment-search">ค้นหารายวิชา</Label>
									<Input
										id="assessment-search"
										class="pl-9"
										placeholder="ค้นหารหัส วิชา หรือผู้รับผิดชอบ"
										bind:value={searchQuery}
									/>
								</div>
								<Select.Root type="single" bind:value={readyFilter}>
									<Select.Trigger class="w-40"
										>{readyOptions.find((option) => option.value === readyFilter)
											?.label}</Select.Trigger
									>
									<Select.Content>
										{#each readyOptions as option (option.value)}<Select.Item value={option.value}
												>{option.label}</Select.Item
											>{/each}
									</Select.Content>
								</Select.Root>
								<Select.Root type="single" bind:value={examFilter}>
									<Select.Trigger class="w-44"
										>{examOptions.find((option) => option.value === examFilter)
											?.label}</Select.Trigger
									>
									<Select.Content>
										{#each examOptions as option (option.value)}<Select.Item value={option.value}
												>{option.label}</Select.Item
											>{/each}
									</Select.Content>
								</Select.Root>
							</div>
						</div>
					</Card.Header>
					<Card.Content class="p-0">
						<div class="overflow-x-auto">
							<Table.Root class="min-w-[1060px]">
								<Table.Header>
									<Table.Row class="bg-muted/40 hover:bg-muted/40">
										<Table.Head class="min-w-64 pl-5">รายวิชา</Table.Head>
										<Table.Head class="min-w-44">ผู้รับผิดชอบ</Table.Head>
										{#each phaseCodes as code (code)}<Table.Head class="min-w-32 text-center"
												>{phaseLabel(code)}</Table.Head
											>{/each}
										<Table.Head class="w-36 text-center">ความพร้อม</Table.Head>
										<Table.Head class="w-12"><span class="sr-only">เปิด</span></Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each filteredPlans as plan (plan.offeringId)}
										<Table.Row class="group cursor-pointer" onclick={() => openPlan(plan)}>
											<Table.Cell class="pl-5">
												<div class="font-medium">{plan.offeringCode} · {plan.offeringName}</div>
												<div class="mt-1 text-xs text-muted-foreground">
													{plan.subjectVersionDisplayLabel} · {plan.learningGroupCount} กลุ่มเรียน
												</div>
											</Table.Cell>
											<Table.Cell>
												{#if plan.assessmentCoordinatorName}
													<div class="flex items-center gap-2 text-sm">
														<UserRoundCheck class="size-4 text-primary" /><span
															>{plan.assessmentCoordinatorName}</span
														>
													</div>
												{:else}<span class="text-sm text-amber-700 dark:text-amber-300"
														>ยังไม่กำหนด</span
													>{/if}
											</Table.Cell>
											{#each phaseCodes as code (code)}
												{@const phase = phaseFor(plan, code)}
												<Table.Cell class="text-center">
													{#if phase}
														<div class="font-semibold tabular-nums">{phase.maxScore}</div>
														{#if code === 'midterm' || code === 'final'}
															<div class="mt-1 text-[11px] text-muted-foreground">
																{arrangementLabel(
																	phase.examArrangement
																)}{#if phase.examDurationMinutes}
																	· {phase.examDurationMinutes} นาที{/if}
															</div>
														{/if}
													{:else}<span class="text-muted-foreground">—</span>{/if}
												</Table.Cell>
											{/each}
											<Table.Cell class="text-center">
												{#if plan.readiness.ready}
													<Badge class="gap-1 bg-emerald-600 text-white hover:bg-emerald-600"
														><Check class="size-3" /> พร้อมใช้</Badge
													>
												{:else}
													<Badge
														variant="outline"
														class="gap-1 border-amber-300 text-amber-800 dark:text-amber-300"
														><CircleAlert class="size-3" />
														{plan.readiness.findings.length} จุด</Badge
													>
												{/if}
												<div class="mt-1 text-xs tabular-nums text-muted-foreground">
													{plan.readiness.totalScore}/{plan.readiness.expectedTotalScore}
												</div>
											</Table.Cell>
											<Table.Cell><ChevronRight class="size-4 text-muted-foreground" /></Table.Cell>
										</Table.Row>
									{/each}
								</Table.Body>
							</Table.Root>
						</div>
						{#if filteredPlans.length === 0}
							<div class="p-10 text-center">
								<BookOpenCheck class="mx-auto mb-3 size-8 text-muted-foreground" />
								<p class="font-medium">ไม่พบรายวิชาตามตัวกรอง</p>
								<p class="mt-1 text-sm text-muted-foreground">ลองเปลี่ยนคำค้นหรือสถานะที่เลือก</p>
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			{/if}
		</div>
	{/if}
</PageShell>

<Sheet.Root bind:open={sheetOpen}>
	<Sheet.Content side="right" class="w-full overflow-y-auto p-0 sm:max-w-2xl">
		{#if detailLoading}
			<div class="flex min-h-full items-center justify-center">
				<Loader2 class="size-7 animate-spin text-primary" />
			</div>
		{:else if detail}
			<Sheet.Header class="sticky top-0 z-10 border-b bg-background/95 px-6 py-5 backdrop-blur">
				<div class="pr-8">
					<Sheet.Title>{detail.offeringCode} · {detail.offeringName}</Sheet.Title>
					<Sheet.Description class="mt-1"
						>{detail.subjectVersionDisplayLabel} · {detail.learningGroupIds.length} กลุ่มเรียน</Sheet.Description
					>
				</div>
				<div class="mt-3 flex items-center justify-between gap-4">
					<div class="flex items-center gap-2 text-sm">
						{#if saveState === 'saving'}<Loader2 class="size-4 animate-spin text-primary" /> กำลังบันทึก…
						{:else if saveState === 'pending'}<Clock3 class="size-4 text-amber-600" /> รอบันทึกอัตโนมัติ
						{:else if saveState === 'saved'}<CloudCheck class="size-4 text-emerald-600" /> บันทึกแล้ว
							{#if lastSavedAt}<span class="text-xs text-muted-foreground"
									>{lastSavedAt.toLocaleTimeString('th-TH')}</span
								>{/if}
						{:else if saveState === 'error'}<CloudAlert class="size-4 text-destructive" /> ยังบันทึกไม่ได้
						{:else}<CloudCheck class="size-4 text-muted-foreground" /> บันทึกอัตโนมัติเมื่อแก้ไข{/if}
					</div>
					<Badge variant="outline" class="tabular-nums"
						>รวม {draftTotal}/{detail.readiness.expectedTotalScore}</Badge
					>
				</div>
			</Sheet.Header>

			<div class="space-y-5 px-6 py-6">
				{#if errorMessage}<div
						class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
					>
						{errorMessage}
					</div>{/if}
				{#if detail.readiness.ready}
					<div
						class="flex items-start gap-3 rounded-xl border border-emerald-200 bg-emerald-50 p-4 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/30 dark:text-emerald-100"
					>
						<ShieldCheck class="mt-0.5 size-5 shrink-0" />
						<div>
							<p class="font-medium">ข้อมูลพร้อมใช้ต่อ</p>
							<p class="mt-1 text-sm opacity-80">
								ระบบตารางสอบจะเห็นเฉพาะช่วงสอบที่พร้อมและเลือก “สอบในตาราง”
							</p>
						</div>
					</div>
				{:else}
					<div
						class="rounded-xl border border-amber-200 bg-amber-50 p-4 text-amber-950 dark:border-amber-900 dark:bg-amber-950/30 dark:text-amber-100"
					>
						<div class="flex items-center gap-2 font-medium">
							<CircleAlert class="size-5" /> จุดที่ต้องตรวจ
						</div>
						<ul class="mt-2 space-y-1 pl-7 text-sm">
							{#each detail.readiness.findings as finding (finding)}<li class="list-disc">
									{findingLabel(finding)}
								</li>{/each}
						</ul>
					</div>
				{/if}

				<section class="space-y-3 rounded-xl border p-4">
					<div class="flex items-start justify-between gap-4">
						<div>
							<div class="flex items-center gap-2 font-medium">
								<UsersRound class="size-4 text-primary" /> ผู้รับผิดชอบโครงสร้างคะแนน
							</div>
							<p class="mt-1 text-xs text-muted-foreground">
								เลือกจากครูที่สอนอย่างน้อยหนึ่งกลุ่มของรายวิชานี้
							</p>
						</div>
						{#if detail.suggestedCoordinatorId && canManageSchool}<Button
								variant="outline"
								size="sm"
								onclick={applySuggestedCoordinator}
								disabled={!canEditDetail}><Sparkles class="size-4" /> ใช้คนที่ระบบแนะนำ</Button
							>{/if}
					</div>
					<Select.Root
						type="single"
						value={draftCoordinatorId || '__none__'}
						disabled={!canEditDetail || !canManageSchool}
						onValueChange={(value) => {
							draftCoordinatorId = value === '__none__' ? '' : value;
							markDirty();
						}}
					>
						<Select.Trigger class="w-full"
							>{detail.coordinatorCandidates.find(
								(candidate) => candidate.teacherId === draftCoordinatorId
							)?.displayName ?? 'ยังไม่ได้กำหนดผู้รับผิดชอบ'}</Select.Trigger
						>
						<Select.Content>
							<Select.Item value="__none__">ยังไม่ได้กำหนด</Select.Item>
							{#each detail.coordinatorCandidates as candidate (candidate.teacherId)}<Select.Item
									value={candidate.teacherId}
									>{candidate.displayName} · ครูหลัก {candidate.primaryLearningGroupCount}/{candidate.learningGroupCount}
									กลุ่ม</Select.Item
								>{/each}
						</Select.Content>
					</Select.Root>
					{#if !canManageSchool && detail.assessmentCoordinatorId === currentUserId}<p
							class="text-xs text-muted-foreground"
						>
							คุณเป็นผู้รับผิดชอบรายวิชานี้ จึงแก้คะแนนทั้ง 4 ช่วงได้
						</p>{/if}
				</section>

				<div class="space-y-3">
					<div>
						<h3 class="font-semibold">คะแนน 4 ช่วงมาตรฐาน</h3>
						<p class="mt-1 text-sm text-muted-foreground">
							คะแนนย่อยของแต่ละห้องจะจัดการในหน้ากรอกคะแนนภายหลัง
						</p>
					</div>
					{#each draftPhases as phase, index (phase.phaseCode)}
						<section class="rounded-xl border bg-card">
							<div class="flex items-center justify-between border-b bg-muted/30 px-4 py-3">
								<div class="flex items-center gap-3">
									<span
										class="flex size-7 items-center justify-center rounded-full bg-primary/10 font-mono text-xs font-semibold text-primary"
										>0{index + 1}</span
									>
									<p class="font-medium">{phaseLabel(phase.phaseCode)}</p>
								</div>
								{#if phase.phaseCode === 'midterm' || phase.phaseCode === 'final'}<Badge
										variant="outline">{arrangementLabel(phase.examArrangement)}</Badge
									>{/if}
							</div>
							<div class="grid gap-4 p-4 sm:grid-cols-2">
								<div class="space-y-2">
									<Label for={`phase-score-${phase.phaseCode}`}>คะแนนเต็ม</Label>
									<Input
										id={`phase-score-${phase.phaseCode}`}
										inputmode="decimal"
										bind:value={phase.maxScore}
										disabled={!canEditDetail}
										oninput={markDirty}
										onblur={flushAutosave}
									/>
								</div>
								{#if phase.phaseCode === 'midterm' || phase.phaseCode === 'final'}
									<div class="space-y-2">
										<Label for={`phase-arrangement-${phase.phaseCode}`}>การจัดสอบ</Label>
										<Select.Root
											type="single"
											value={phase.examArrangement}
											disabled={!canEditDetail}
											onValueChange={(value) =>
												updateArrangement(phase, value as AssessmentExamArrangement)}
										>
											<Select.Trigger id={`phase-arrangement-${phase.phaseCode}`} class="w-full"
												>{arrangementOptions.find(
													(option) => option.value === phase.examArrangement
												)?.label}</Select.Trigger
											>
											<Select.Content
												>{#each arrangementOptions as option (option.value)}<Select.Item
														value={option.value}>{option.label}</Select.Item
													>{/each}</Select.Content
											>
										</Select.Root>
									</div>
									{#if phase.examArrangement !== 'none'}
										<div class="space-y-2 sm:col-start-2">
											<Label for={`phase-duration-${phase.phaseCode}`}>เวลาสอบ (นาที)</Label><Input
												id={`phase-duration-${phase.phaseCode}`}
												type="number"
												min="1"
												placeholder="เช่น 60"
												bind:value={phase.examDurationMinutes}
												disabled={!canEditDetail}
												oninput={markDirty}
												onblur={flushAutosave}
											/>
										</div>
									{/if}
								{/if}
							</div>
						</section>
					{/each}
				</div>

				{#if !canEditDetail}<div
						class="rounded-xl border border-dashed p-4 text-sm text-muted-foreground"
					>
						ดูข้อมูลได้ แต่การแก้ไขทำได้โดยผู้รับผิดชอบโครงสร้างคะแนนรายวิชาหรือผู้ดูแลวิชาการ
					</div>{/if}
			</div>
		{/if}
	</Sheet.Content>
</Sheet.Root>
