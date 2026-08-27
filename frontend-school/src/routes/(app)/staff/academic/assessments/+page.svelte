<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAcademicContextStore,
		registerAcademicContextDirtySource
	} from '$lib/academic-context/store';
	import {
		getAssessmentPlan,
		getAssessmentSettings,
		listAssessmentPlans,
		saveAssessmentPlan,
		submitAssessmentPlan,
		updateAssessmentSettings,
		type AssessmentPlanDetail,
		type AssessmentPlanStatus,
		type AssessmentPlanSummary,
		type SaveAssessmentCategoryRequest
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
	import { Switch } from '$lib/components/ui/switch';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		BookOpenCheck,
		CheckCircle2,
		ChevronRight,
		CirclePlus,
		Loader2,
		Save,
		Send,
		Trash2
	} from 'lucide-svelte';

	type StatusFilter = AssessmentPlanStatus | 'all';

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	let plans = $state<AssessmentPlanSummary[]>([]);
	let detail = $state<AssessmentPlanDetail | null>(null);
	let draftCategories = $state<SaveAssessmentCategoryRequest[]>([]);
	let selectedOfferingId = $state('');
	let statusFilter = $state<StatusFilter>('all');
	let teacherAccessEnabled = $state(true);
	let loading = $state(false);
	let detailLoading = $state(false);
	let busy = $state(false);
	let dirty = $state(false);
	let errorMessage = $state('');
	let revision = 0;

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
	const canManage = $derived(
		canManageSchool ||
			(teacherAccessEnabled && $can.has(PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED))
	);
	const canEditDetail = $derived(
		canManage && detail?.status !== 'submitted' && detail?.status !== 'locked'
	);
	const filteredPlans = $derived(
		statusFilter === 'all' ? plans : plans.filter((plan) => plan.status === statusFilter)
	);
	const configuredCount = $derived(plans.filter((plan) => plan.status !== 'not_configured').length);
	const readyCount = $derived(
		plans.filter((plan) => plan.totalScore === plan.expectedTotalScore).length
	);

	const statusOptions: Array<{ value: StatusFilter; label: string }> = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'not_configured', label: 'ยังไม่ตั้งค่า' },
		{ value: 'draft', label: 'ฉบับร่าง' },
		{ value: 'saved', label: 'บันทึกแล้ว' },
		{ value: 'submitted', label: 'ส่งแล้ว' },
		{ value: 'locked', label: 'ล็อกแล้ว' }
	];
	const examModeOptions = [
		{ value: 'none', label: 'ไม่ใช่การสอบ' },
		{ value: 'in_timetable', label: 'สอบในตาราง' },
		{ value: 'outside_timetable', label: 'สอบนอกตาราง' },
		{ value: 'practical', label: 'ปฏิบัติ / ชิ้นงาน' }
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

	function statusLabel(status: AssessmentPlanStatus): string {
		return statusOptions.find((option) => option.value === status)?.label ?? status;
	}

	function statusVariant(
		status: AssessmentPlanStatus
	): 'default' | 'secondary' | 'outline' | 'destructive' {
		if (status === 'submitted') return 'default';
		if (status === 'locked') return 'secondary';
		if (status === 'not_configured') return 'outline';
		return 'secondary';
	}

	function cloneCategories(source: AssessmentPlanDetail): SaveAssessmentCategoryRequest[] {
		return source.categories.map((category) => ({
			id: category.id ?? null,
			code: category.code ?? null,
			name: category.name,
			maxScore: category.maxScore,
			examMode: category.examMode,
			examDurationMinutes: category.examDurationMinutes ?? null,
			displayOrder: category.displayOrder,
			items: category.items.map((item) => ({
				id: item.id,
				name: item.name,
				maxScore: item.maxScore,
				displayOrder: item.displayOrder,
				isActive: item.isActive
			}))
		}));
	}

	function markDirty(): void {
		dirty = true;
	}

	async function loadWorkspace(termId: string): Promise<void> {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const settings = await getAssessmentSettings();
			if (current !== revision) return;
			teacherAccessEnabled = settings.teacherAccessEnabled;
			const rows = await listAssessmentPlans({ academicTermId: termId });
			if (current !== revision) return;
			plans = rows;
			selectedOfferingId = '';
			detail = null;
			draftCategories = [];
			dirty = false;
		} catch (error) {
			if (current === revision) {
				errorMessage = error instanceof Error ? error.message : 'โหลดโครงสร้างคะแนนไม่สำเร็จ';
			}
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function openPlan(plan: AssessmentPlanSummary): Promise<void> {
		if (dirty && plan.offeringId !== selectedOfferingId) {
			toast.warning('กรุณาบันทึกการแก้ไขก่อนเปิดรายการเปิดสอนอื่น');
			return;
		}
		selectedOfferingId = plan.offeringId;
		detailLoading = true;
		errorMessage = '';
		try {
			const loaded = await getAssessmentPlan(plan.offeringId);
			detail = loaded;
			draftCategories = cloneCategories(loaded);
			dirty = false;
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดรายละเอียดโครงสร้างคะแนนไม่สำเร็จ';
		} finally {
			detailLoading = false;
		}
	}

	function addCategory(): void {
		draftCategories.push({
			id: null,
			code: null,
			name: '',
			maxScore: '0',
			examMode: 'none',
			examDurationMinutes: null,
			displayOrder: draftCategories.length + 1,
			items: []
		});
		markDirty();
	}

	function removeCategory(index: number): void {
		draftCategories.splice(index, 1);
		markDirty();
	}

	function addItem(category: SaveAssessmentCategoryRequest): void {
		category.items.push({
			id: null,
			name: '',
			maxScore: '0',
			displayOrder: category.items.length + 1,
			isActive: true
		});
		markDirty();
	}

	function removeItem(category: SaveAssessmentCategoryRequest, itemIndex: number): void {
		category.items.splice(itemIndex, 1);
		markDirty();
	}

	async function refreshPlans(): Promise<void> {
		if (!academicTermId) return;
		plans = await listAssessmentPlans({ academicTermId });
	}

	async function savePlan(): Promise<void> {
		if (!detail) return;
		busy = true;
		errorMessage = '';
		try {
			const saved = await saveAssessmentPlan(detail.offeringId, {
				rowVersion: detail.rowVersion ?? null,
				categories: draftCategories.map((category, categoryIndex) => ({
					...category,
					code: category.code?.trim() || null,
					name: category.name.trim(),
					displayOrder: categoryIndex + 1,
					examDurationMinutes: category.examMode === 'none' ? null : category.examDurationMinutes,
					items: category.items.map((item, itemIndex) => ({
						...item,
						name: item.name.trim(),
						displayOrder: itemIndex + 1
					}))
				}))
			});
			detail = saved;
			draftCategories = cloneCategories(saved);
			dirty = false;
			await refreshPlans();
			toast.success('บันทึกโครงสร้างคะแนนแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกโครงสร้างคะแนนไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	async function submitPlan(): Promise<void> {
		if (!detail || dirty) return;
		busy = true;
		errorMessage = '';
		try {
			const submitted = await submitAssessmentPlan(detail.offeringId);
			detail = submitted;
			draftCategories = cloneCategories(submitted);
			await refreshPlans();
			toast.success('ส่งโครงสร้างคะแนนแล้ว');
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ส่งโครงสร้างคะแนนไม่สำเร็จ';
			toast.error(errorMessage);
		} finally {
			busy = false;
		}
	}

	async function toggleTeacherAccess(): Promise<void> {
		if (!canManageSchool) return;
		busy = true;
		try {
			const settings = await updateAssessmentSettings({
				teacherAccessEnabled: !teacherAccessEnabled
			});
			teacherAccessEnabled = settings.teacherAccessEnabled;
			toast.success('บันทึกสิทธิ์การจัดทำโครงสร้างคะแนนแล้ว');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'บันทึกการตั้งค่าไม่สำเร็จ');
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		const unregisterDirty = registerAcademicContextDirtySource(
			'academic-assessment-plan',
			() => dirty
		);
		const unsubscribe = academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadWorkspace(termId);
			}
		});
		return () => {
			unsubscribe();
			unregisterDirty();
		};
	});
</script>

<PageShell
	title="โครงสร้างคะแนน"
	description="กำหนดหมวดคะแนนและงานย่อยแยกตามรายการเปิดสอนของภาคเรียนที่เลือก"
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
		<PageSkeleton variant="cards" rows={5} />
	{:else if errorMessage && plans.length === 0}
		<PageState
			variant="error"
			title="โหลดโครงสร้างคะแนนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicTermId)}
		/>
	{:else}
		<div class="space-y-6">
			<div class="grid gap-4 md:grid-cols-3">
				<Card.Root class="border-primary/20 bg-primary/5">
					<Card.Header class="pb-3">
						<Card.Description>รายการเปิดสอนทั้งหมด</Card.Description>
						<Card.Title class="text-3xl">{plans.length}</Card.Title>
					</Card.Header>
				</Card.Root>
				<Card.Root>
					<Card.Header class="pb-3">
						<Card.Description>ตั้งค่าแล้ว</Card.Description>
						<Card.Title class="text-3xl">{configuredCount}</Card.Title>
					</Card.Header>
				</Card.Root>
				<Card.Root>
					<Card.Header class="pb-3">
						<Card.Description>คะแนนรวมตรงตามเกณฑ์</Card.Description>
						<Card.Title class="text-3xl">{readyCount}</Card.Title>
					</Card.Header>
				</Card.Root>
			</div>

			{#if plans.length === 0}
				<AcademicPrerequisiteNotice prerequisite={noCourseOfferingPrerequisite} />
			{/if}

			{#if canManageSchool}
				<Card.Root class="gap-0 py-0">
					<Card.Content class="flex flex-wrap items-center justify-between gap-4 pt-6">
						<div>
							<p class="font-medium">เปิดให้ครูผู้สอนจัดทำโครงสร้างคะแนน</p>
							<p class="text-muted-foreground text-sm">
								มีผลกับครูที่ได้รับมอบหมายในรายการเปิดสอนเท่านั้น
							</p>
						</div>
						<div class="flex items-center gap-3">
							<Label for="teacher-access">{teacherAccessEnabled ? 'เปิด' : 'ปิด'}</Label>
							<Switch
								id="teacher-access"
								checked={teacherAccessEnabled}
								disabled={busy}
								onclick={toggleTeacherAccess}
							/>
						</div>
					</Card.Content>
				</Card.Root>
			{/if}

			<div class="grid items-start gap-6 xl:grid-cols-[minmax(20rem,0.8fr)_minmax(0,1.7fr)]">
				<Card.Root>
					<Card.Header>
						<div class="flex items-center justify-between gap-3">
							<div>
								<Card.Title>รายการเปิดสอน</Card.Title>
								<Card.Description>เลือกชุดที่ต้องการกำหนดคะแนน</Card.Description>
							</div>
							<label class="sr-only" for="assessment-status">กรองสถานะ</label>
							<Select.Root type="single" bind:value={statusFilter}>
								<Select.Trigger id="assessment-status" class="w-40">
									{statusOptions.find((option) => option.value === statusFilter)?.label ??
										'ทุกสถานะ'}
								</Select.Trigger>
								<Select.Content>
									{#each statusOptions as option (option.value)}
										<Select.Item value={option.value}>{option.label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					</Card.Header>
					<Card.Content class="space-y-2">
						{#if filteredPlans.length === 0}
							<div class="border-border rounded-lg border border-dashed p-8 text-center">
								<BookOpenCheck class="text-muted-foreground mx-auto mb-3 size-8" />
								<p class="font-medium">ยังไม่มีรายการเปิดสอนในสถานะนี้</p>
								<p class="text-muted-foreground mt-1 text-sm">
									สร้างและเผยแพร่รายการเปิดสอนก่อนกำหนดโครงสร้างคะแนน
								</p>
							</div>
						{:else}
							{#each filteredPlans as plan (plan.offeringId)}
								<button
									type="button"
									class="hover:bg-muted/60 focus-visible:ring-ring flex w-full items-center gap-3 rounded-lg border p-3 text-left transition focus-visible:ring-2 focus-visible:outline-none {selectedOfferingId ===
									plan.offeringId
										? 'border-primary bg-primary/5'
										: 'border-border'}"
									onclick={() => openPlan(plan)}
								>
									<div class="min-w-0 flex-1">
										<div class="flex flex-wrap items-center gap-2">
											<span class="truncate font-medium"
												>{plan.offeringCode} · {plan.offeringName}</span
											>
											<Badge variant={statusVariant(plan.status)}>{statusLabel(plan.status)}</Badge>
										</div>
										<p class="text-muted-foreground mt-1 truncate text-sm">
											{plan.subjectVersionDisplayLabel} · {plan.learningGroupCount} กลุ่มเรียน
										</p>
										<p class="mt-2 text-sm tabular-nums">
											{plan.totalScore} / {plan.expectedTotalScore} คะแนน
										</p>
									</div>
									<ChevronRight class="text-muted-foreground size-4" />
								</button>
							{/each}
						{/if}
					</Card.Content>
				</Card.Root>

				<Card.Root>
					{#if detailLoading}
						<Card.Content class="flex min-h-80 items-center justify-center">
							<Loader2 class="text-primary size-7 animate-spin" />
						</Card.Content>
					{:else if !detail}
						<Card.Content class="flex min-h-80 flex-col items-center justify-center text-center">
							<BookOpenCheck class="text-muted-foreground mb-4 size-10" />
							<p class="font-medium">เลือกรายการเปิดสอนเพื่อเริ่มจัดโครงสร้าง</p>
							<p class="text-muted-foreground mt-1 max-w-md text-sm">
								คะแนนจะผูกกับรายการเปิดสอนและภาคเรียนโดยตรง ไม่ผูกกับห้องเรียนแบบเดิม
							</p>
						</Card.Content>
					{:else}
						<Card.Header class="border-b">
							<div class="flex flex-wrap items-start justify-between gap-4">
								<div>
									<div class="flex flex-wrap items-center gap-2">
										<Card.Title>{detail.offeringCode} · {detail.offeringName}</Card.Title>
										<Badge variant={statusVariant(detail.status)}
											>{statusLabel(detail.status)}</Badge
										>
									</div>
									<Card.Description class="mt-2">
										{detail.subjectVersionDisplayLabel} · เป้าหมาย {detail.expectedTotalScore} คะแนน
									</Card.Description>
								</div>
								{#if canManage}
									<div class="flex flex-wrap gap-2">
										<Button
											variant="outline"
											disabled={busy || !dirty || !canEditDetail}
											onclick={savePlan}
										>
											{#if busy}<Loader2 class="animate-spin" />{:else}<Save />{/if}
											บันทึก
										</Button>
										<Button
											disabled={busy || dirty || detail.status !== 'saved'}
											onclick={submitPlan}
										>
											<Send /> ส่งโครงสร้าง
										</Button>
									</div>
								{/if}
							</div>
							{#if dirty}
								<p class="text-amber-700 dark:text-amber-300 text-sm">
									มีการแก้ไขที่ยังไม่บันทึก — ต้องบันทึกก่อนเปลี่ยนปี ภาคเรียน หรือรายการเปิดสอน
								</p>
							{/if}
						</Card.Header>
						<Card.Content class="space-y-5 pt-6">
							{#if errorMessage}
								<div
									class="border-destructive/30 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm"
								>
									{errorMessage}
								</div>
							{/if}

							{#each draftCategories as category, categoryIndex (category.id ?? categoryIndex)}
								<section class="border-border rounded-xl border p-4">
									<div class="grid gap-4 md:grid-cols-[8rem_minmax(12rem,1fr)_7rem_11rem_auto]">
										<div class="space-y-2">
											<Label for={`category-code-${categoryIndex}`}>รหัส</Label>
											<Input
												id={`category-code-${categoryIndex}`}
												bind:value={category.code}
												disabled={!canEditDetail}
												oninput={markDirty}
											/>
										</div>
										<div class="space-y-2">
											<Label for={`category-name-${categoryIndex}`}>ชื่อหมวดคะแนน</Label>
											<Input
												id={`category-name-${categoryIndex}`}
												bind:value={category.name}
												disabled={!canEditDetail}
												oninput={markDirty}
											/>
										</div>
										<div class="space-y-2">
											<Label for={`category-score-${categoryIndex}`}>คะแนนเต็ม</Label>
											<Input
												id={`category-score-${categoryIndex}`}
												inputmode="decimal"
												bind:value={category.maxScore}
												disabled={!canEditDetail}
												oninput={markDirty}
											/>
										</div>
										<div class="space-y-2">
											<Label for={`category-mode-${categoryIndex}`}>รูปแบบ</Label>
											<Select.Root
												type="single"
												bind:value={category.examMode}
												disabled={!canEditDetail}
												onValueChange={markDirty}
											>
												<Select.Trigger id={`category-mode-${categoryIndex}`} class="w-full">
													{examModeOptions.find((option) => option.value === category.examMode)
														?.label ?? 'เลือกรูปแบบ'}
												</Select.Trigger>
												<Select.Content>
													{#each examModeOptions as option (option.value)}
														<Select.Item value={option.value}>{option.label}</Select.Item>
													{/each}
												</Select.Content>
											</Select.Root>
										</div>
										<div class="flex items-end justify-end">
											<Button
												variant="ghost"
												size="icon"
												disabled={!canEditDetail}
												title="ลบหมวดคะแนน"
												onclick={() => removeCategory(categoryIndex)}
											>
												<Trash2 />
											</Button>
										</div>
									</div>

									{#if category.examMode !== 'none'}
										<div class="mt-4 max-w-48 space-y-2">
											<Label for={`category-duration-${categoryIndex}`}>เวลาสอบ (นาที)</Label>
											<Input
												id={`category-duration-${categoryIndex}`}
												type="number"
												min="1"
												bind:value={category.examDurationMinutes}
												disabled={!canEditDetail}
												oninput={markDirty}
											/>
										</div>
									{/if}

									<div class="bg-muted/40 mt-4 space-y-3 rounded-lg p-3">
										<div class="flex items-center justify-between gap-3">
											<div>
												<p class="text-sm font-medium">รายการเก็บคะแนน</p>
												<p class="text-muted-foreground text-xs">
													ผลรวมรายการควรตรงกับคะแนนเต็มของหมวด
												</p>
											</div>
											<Button
												variant="outline"
												size="sm"
												disabled={!canEditDetail}
												onclick={() => addItem(category)}
											>
												<CirclePlus /> เพิ่มรายการ
											</Button>
										</div>
										{#each category.items as item, itemIndex (item.id ?? itemIndex)}
											<div class="grid gap-3 sm:grid-cols-[minmax(12rem,1fr)_7rem_auto]">
												<div>
													<Label class="sr-only" for={`item-name-${categoryIndex}-${itemIndex}`}
														>ชื่อรายการ</Label
													>
													<Input
														id={`item-name-${categoryIndex}-${itemIndex}`}
														placeholder="เช่น แบบฝึกหัดครั้งที่ 1"
														bind:value={item.name}
														disabled={!canEditDetail}
														oninput={markDirty}
													/>
												</div>
												<div>
													<Label class="sr-only" for={`item-score-${categoryIndex}-${itemIndex}`}
														>คะแนน</Label
													>
													<Input
														id={`item-score-${categoryIndex}-${itemIndex}`}
														inputmode="decimal"
														bind:value={item.maxScore}
														disabled={!canEditDetail}
														oninput={markDirty}
													/>
												</div>
												<Button
													variant="ghost"
													size="icon"
													disabled={!canEditDetail}
													title="ลบรายการ"
													onclick={() => removeItem(category, itemIndex)}
												>
													<Trash2 />
												</Button>
											</div>
										{/each}
									</div>
								</section>
							{/each}

							{#if canEditDetail}
								<Button variant="outline" class="w-full border-dashed" onclick={addCategory}>
									<CirclePlus /> เพิ่มหมวดคะแนน
								</Button>
							{:else if detail.status === 'submitted' || detail.status === 'locked'}
								<div class="bg-muted flex items-center gap-2 rounded-lg p-3 text-sm">
									<CheckCircle2 class="text-primary size-4" />
									โครงสร้างนี้ส่งแล้วและไม่สามารถแก้ไขจากหน้านี้
								</div>
							{/if}
						</Card.Content>
					{/if}
				</Card.Root>
			</div>
		</div>
	{/if}
</PageShell>
