<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		getAssessmentPlan,
		listAssessmentPlans,
		saveAssessmentPlan,
		submitAssessmentPlan,
		type AssessmentExamMode,
		type AssessmentPlanDetail,
		type AssessmentPlanStatus,
		type AssessmentPlanSummary,
		type SaveAssessmentCategoryRequest
	} from '$lib/api/academicAssessments';
	import {
		getAcademicStructure,
		listClassrooms,
		type AcademicStructureData,
		type Classroom
	} from '$lib/api/academic';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		AlertTriangle,
		ClipboardList,
		Download,
		FileSpreadsheet,
		Loader2,
		Pencil,
		Plus,
		Save,
		Send,
		Trash2
	} from 'lucide-svelte';

	let { data } = $props();

	type StatusFilter = AssessmentPlanStatus | 'all';
	type EditorItem = {
		clientId: string;
		id?: string;
		name: string;
		maxScore: number;
		displayOrder: number;
		isActive: boolean;
	};
	type EditorCategory = Omit<SaveAssessmentCategoryRequest, 'items'> & {
		clientId: string;
		items: EditorItem[];
	};

	const canReadAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_READ_ALL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL
		)
	);
	const canManageAssessment = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
			PERMISSIONS.ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL
		)
	);

	const examModeOptions: { value: AssessmentExamMode; label: string }[] = [
		{ value: 'none', label: 'ไม่ใช่การสอบ' },
		{ value: 'in_timetable', label: 'สอบในตาราง' },
		{ value: 'outside_timetable', label: 'สอบนอกตาราง' },
		{ value: 'practical', label: 'ปฏิบัติ/ชิ้นงาน' }
	];

	const statusOptions: { value: StatusFilter; label: string }[] = [
		{ value: 'all', label: 'ทุกสถานะ' },
		{ value: 'not_configured', label: 'ยังไม่ตั้งค่า' },
		{ value: 'draft', label: 'ร่าง' },
		{ value: 'submitted', label: 'ส่งแล้ว' },
		{ value: 'locked', label: 'ล็อกแล้ว' }
	];

	let loading = $state(true);
	let loadingPlans = $state(false);
	let exporting = $state(false);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let plans = $state<AssessmentPlanSummary[]>([]);
	let selectedYearId = $state('');
	let selectedSemesterId = $state('');
	let selectedClassroomId = $state('');
	let selectedStatus = $state<StatusFilter>('all');
	let editorOpen = $state(false);
	let editorLoading = $state(false);
	let saving = $state(false);
	let submitting = $state(false);
	let editingCourse = $state<AssessmentPlanSummary | null>(null);
	let editingPlan = $state<AssessmentPlanDetail | null>(null);
	let editorCategories = $state<EditorCategory[]>([]);
	let localIdCounter = 0;

	const filteredSemesters = $derived(
		structure.semesters.filter((semester) => semester.academic_year_id === selectedYearId)
	);

	const summary = $derived({
		total: plans.length,
		draft: plans.filter((plan) => plan.status === 'draft' || plan.status === 'not_configured')
			.length,
		submitted: plans.filter((plan) => plan.status === 'submitted').length,
		locked: plans.filter((plan) => plan.status === 'locked').length,
		outside: plans.reduce((total, plan) => total + plan.outsideTimetableCount, 0),
		unallocated: plans.filter((plan) => plan.hasUnallocatedCategories).length
	});

	function nextClientId(prefix: string) {
		localIdCounter += 1;
		return `${prefix}-${localIdCounter}`;
	}

	function statusLabel(status: AssessmentPlanStatus) {
		return statusOptions.find((option) => option.value === status)?.label ?? status;
	}

	function statusBadgeVariant(status: AssessmentPlanStatus) {
		if (status === 'submitted') return 'default';
		if (status === 'locked') return 'secondary';
		if (status === 'not_configured') return 'outline';
		return 'secondary';
	}

	function examModeLabel(mode: AssessmentExamMode | string) {
		return examModeOptions.find((option) => option.value === mode)?.label ?? mode;
	}

	function allocationStatusLabel(status: string) {
		if (status === 'complete') return 'ครบ';
		if (status === 'under_allocated') return 'ยังไม่ครบ';
		if (status === 'over_allocated') return 'เกิน';
		return 'ยังไม่เริ่ม';
	}

	function courseTitle(plan: AssessmentPlanSummary) {
		const subject = [plan.subjectCode, plan.subjectNameTh || plan.subjectNameEn]
			.filter(Boolean)
			.join(' ');
		return subject || 'รายวิชา';
	}

	function categoryTotal(category: EditorCategory) {
		return category.items
			.filter((item) => item.isActive)
			.reduce((total, item) => total + Number(item.maxScore || 0), 0);
	}

	async function initData() {
		if (!canReadAssessment) {
			loading = false;
			return;
		}
		loading = true;
		try {
			const structureResponse = await getAcademicStructure();
			structure = structureResponse.data;
			const activeYear = structure.years.find((year) => year.is_active) ?? structure.years[0];
			selectedYearId = activeYear?.id ?? '';
			const firstSemester =
				structure.semesters.find(
					(semester) => semester.academic_year_id === selectedYearId && semester.is_active
				) ?? structure.semesters.find((semester) => semester.academic_year_id === selectedYearId);
			selectedSemesterId = firstSemester?.id ?? '';
			await loadClassrooms();
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function loadClassrooms() {
		if (!selectedYearId) {
			classrooms = [];
			return;
		}
		const response = await listClassrooms({ year_id: selectedYearId });
		classrooms = response.data ?? [];
	}

	async function loadPlans() {
		if (!canReadAssessment) return;
		loadingPlans = true;
		try {
			const response = await listAssessmentPlans({
				academicSemesterId: selectedSemesterId || undefined,
				classroomId: selectedClassroomId || undefined,
				status: selectedStatus === 'all' ? undefined : selectedStatus
			});
			plans = response.data;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดโครงสร้างคะแนนได้');
		} finally {
			loadingPlans = false;
		}
	}

	async function onYearChange(yearId: string) {
		selectedYearId = yearId;
		selectedClassroomId = '';
		const firstSemester = structure.semesters.find(
			(semester) => semester.academic_year_id === yearId
		);
		selectedSemesterId = firstSemester?.id ?? '';
		await loadClassrooms();
		await loadPlans();
	}

	async function openPlanEditor(course: AssessmentPlanSummary) {
		if (!canManageAssessment) return;
		editingCourse = course;
		editorOpen = true;
		editorLoading = true;
		try {
			const response = await getAssessmentPlan(course.classroomCourseId);
			editingPlan = response.data;
			editorCategories = response.data.categories.map((category) => ({
				clientId: nextClientId('category'),
				id: category.id,
				code: category.code,
				name: category.name,
				maxScore: category.maxScore,
				examMode: category.examMode,
				displayOrder: category.displayOrder,
				items: category.items.map((item) => ({
					clientId: nextClientId('item'),
					id: item.id,
					name: item.name,
					maxScore: item.maxScore,
					displayOrder: item.displayOrder,
					isActive: item.isActive
				}))
			}));
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถเปิดโครงสร้างคะแนนได้');
			editorOpen = false;
		} finally {
			editorLoading = false;
		}
	}

	function addCategory() {
		editorCategories.push({
			clientId: nextClientId('category'),
			code: 'custom',
			name: 'หมวดคะแนนใหม่',
			maxScore: 0,
			examMode: 'none',
			displayOrder: (editorCategories.length + 1) * 10,
			items: []
		});
	}

	function removeCategory(index: number) {
		editorCategories.splice(index, 1);
		reorderCategories();
	}

	function addItem(categoryIndex: number) {
		const category = editorCategories[categoryIndex];
		category.items.push({
			clientId: nextClientId('item'),
			name: 'รายการคะแนนใหม่',
			maxScore: 0,
			displayOrder: (category.items.length + 1) * 10,
			isActive: true
		});
	}

	function removeItem(categoryIndex: number, itemIndex: number) {
		editorCategories[categoryIndex].items.splice(itemIndex, 1);
		editorCategories[categoryIndex].items.forEach((item, index) => {
			item.displayOrder = (index + 1) * 10;
		});
	}

	function reorderCategories() {
		editorCategories.forEach((category, index) => {
			category.displayOrder = (index + 1) * 10;
		});
	}

	function buildPayload() {
		reorderCategories();
		return {
			categories: editorCategories.map((category) => ({
				id: category.id,
				code: category.code,
				name: category.name.trim(),
				maxScore: Number(category.maxScore || 0),
				examMode: category.examMode,
				displayOrder: category.displayOrder,
				items: category.items.map((item, index) => ({
					id: item.id,
					name: item.name.trim(),
					maxScore: Number(item.maxScore || 0),
					displayOrder: (index + 1) * 10,
					isActive: item.isActive
				}))
			}))
		};
	}

	async function saveEditor() {
		if (!editingCourse) return;
		saving = true;
		try {
			const response = await saveAssessmentPlan(editingCourse.classroomCourseId, buildPayload());
			editingPlan = response.data;
			toast.success('บันทึกโครงสร้างคะแนนแล้ว');
			await loadPlans();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกโครงสร้างคะแนนได้');
		} finally {
			saving = false;
		}
	}

	async function submitEditor() {
		if (!editingCourse) return;
		submitting = true;
		try {
			await saveAssessmentPlan(editingCourse.classroomCourseId, buildPayload());
			const response = await submitAssessmentPlan(editingCourse.classroomCourseId);
			editingPlan = response.data;
			toast.success('ส่งโครงสร้างคะแนนแล้ว');
			await loadPlans();
			editorOpen = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถส่งโครงสร้างคะแนนได้');
		} finally {
			submitting = false;
		}
	}

	async function exportAssessmentReport(kind: 'overview' | 'exam') {
		if (plans.length === 0) {
			toast.error('ไม่มีข้อมูลสำหรับดาวน์โหลด');
			return;
		}
		exporting = true;
		try {
			const XLSX = await import('xlsx');
			const rows = plans
				.filter(
					(plan) =>
						kind === 'overview' || plan.inTimetableCount > 0 || plan.outsideTimetableCount > 0
				)
				.map((plan) => ({
					ห้องเรียน: plan.classroomName ?? '',
					รหัสวิชา: plan.subjectCode ?? '',
					รายวิชา: plan.subjectNameTh ?? plan.subjectNameEn ?? '',
					ครูผู้สอน: plan.instructorName ?? '',
					สถานะ: statusLabel(plan.status),
					คะแนนรวม: plan.totalScore,
					จำนวนหมวด: plan.categoryCount,
					จำนวนคะแนนย่อย: plan.itemCount,
					สอบในตาราง: plan.inTimetableCount,
					สอบนอกตาราง: plan.outsideTimetableCount,
					คะแนนย่อยไม่ลงตัว: plan.hasUnallocatedCategories ? 'ใช่' : 'ไม่ใช่'
				}));
			const worksheet = XLSX.utils.json_to_sheet(rows);
			const workbook = XLSX.utils.book_new();
			XLSX.utils.book_append_sheet(workbook, worksheet, kind === 'overview' ? 'Overview' : 'Exams');
			XLSX.writeFile(
				workbook,
				`โครงสร้างคะแนน-${kind === 'overview' ? 'ภาพรวม' : 'รูปแบบการสอบ'}.xlsx`
			);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถดาวน์โหลดเอกสารได้');
		} finally {
			exporting = false;
		}
	}

	onMount(initData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="โครงสร้างคะแนน"
	description="กำหนดคะแนนก่อนกลางภาค กลางภาค หลังกลางภาค ปลายภาค และรูปแบบการสอบของรายวิชาที่เปิดสอน"
>
	{#snippet actions()}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				<Button variant="outline" size="sm" disabled={exporting || plans.length === 0}>
					{#if exporting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{:else}
						<Download class="mr-2 h-4 w-4" />
					{/if}
					ดาวน์โหลด
				</Button>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end">
				<DropdownMenu.Item onclick={() => exportAssessmentReport('overview')}>
					<FileSpreadsheet class="mr-2 h-4 w-4" />
					ภาพรวมโครงสร้างคะแนน
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={() => exportAssessmentReport('exam')}>
					<ClipboardList class="mr-2 h-4 w-4" />
					รายวิชาที่มีการสอบ
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/snippet}

	{#if !canReadAssessment}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูโครงสร้างคะแนน"
			description="บัญชีนี้ยังไม่มีสิทธิ์ดูโครงสร้างคะแนนรายวิชาที่รับผิดชอบหรือภาพรวมทั้งโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="table" rows={6} columns={7} />
	{:else}
		<div class="space-y-5">
			<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">รายวิชา</p>
					<p class="mt-2 text-2xl font-semibold">{summary.total}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">ร่าง/ยังไม่ตั้งค่า</p>
					<p class="mt-2 text-2xl font-semibold">{summary.draft}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">ส่งแล้ว</p>
					<p class="mt-2 text-2xl font-semibold">{summary.submitted}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">สอบนอกตาราง</p>
					<p class="mt-2 text-2xl font-semibold">{summary.outside}</p>
				</div>
				<div class="rounded-lg border bg-background p-4">
					<p class="text-sm text-muted-foreground">คะแนนย่อยไม่ลงตัว</p>
					<p class="mt-2 text-2xl font-semibold">{summary.unallocated}</p>
				</div>
			</div>

			<div class="rounded-lg border bg-background p-4">
				<div class="grid gap-4 lg:grid-cols-[1fr_1fr_1fr_160px_auto]">
					<div class="space-y-2">
						<Label>ปีการศึกษา</Label>
						<Select.Root type="single" value={selectedYearId} onValueChange={onYearChange}>
							<Select.Trigger>
								{structure.years.find((year) => year.id === selectedYearId)?.name ?? 'เลือกปี'}
							</Select.Trigger>
							<Select.Content>
								{#each structure.years as year (year.id)}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>ภาคเรียน</Label>
						<Select.Root type="single" bind:value={selectedSemesterId}>
							<Select.Trigger>
								{filteredSemesters.find((semester) => semester.id === selectedSemesterId)?.name ??
									'ทุกภาคเรียน'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ทุกภาคเรียน</Select.Item>
								{#each filteredSemesters as semester (semester.id)}
									<Select.Item value={semester.id}
										>เทอม {semester.term} ({semester.name})</Select.Item
									>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>ห้องเรียน</Label>
						<Select.Root type="single" bind:value={selectedClassroomId}>
							<Select.Trigger>
								{classrooms.find((classroom) => classroom.id === selectedClassroomId)?.name ??
									'ทุกห้องเรียน'}
							</Select.Trigger>
							<Select.Content class="max-h-[320px]">
								<Select.Item value="">ทุกห้องเรียน</Select.Item>
								{#each classrooms as classroom (classroom.id)}
									<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>สถานะ</Label>
						<Select.Root type="single" bind:value={selectedStatus}>
							<Select.Trigger
								>{statusOptions.find((option) => option.value === selectedStatus)
									?.label}</Select.Trigger
							>
							<Select.Content>
								{#each statusOptions as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="flex items-end">
						<Button class="w-full" onclick={loadPlans} disabled={loadingPlans}>
							{#if loadingPlans}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							ค้นหา
						</Button>
					</div>
				</div>
			</div>

			<div class="overflow-hidden rounded-lg border bg-background">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head>รายวิชา</Table.Head>
							<Table.Head>ห้องเรียน</Table.Head>
							<Table.Head>ครูผู้สอน</Table.Head>
							<Table.Head class="text-right">คะแนนรวม</Table.Head>
							<Table.Head>รูปแบบสอบ</Table.Head>
							<Table.Head>สถานะ</Table.Head>
							<Table.Head class="w-[96px] text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loadingPlans}
							<Table.Row>
								<Table.Cell colspan={7} class="h-24 text-center text-muted-foreground">
									<Loader2 class="mx-auto mb-2 h-5 w-5 animate-spin" />
									กำลังโหลดข้อมูล
								</Table.Cell>
							</Table.Row>
						{:else if plans.length === 0}
							<Table.Row>
								<Table.Cell colspan={7}>
									<PageState
										variant="empty"
										title="ยังไม่มีรายวิชาตามตัวกรองนี้"
										description="เลือกปี ภาคเรียน หรือห้องเรียนอื่นเพื่อดูโครงสร้างคะแนน"
									/>
								</Table.Cell>
							</Table.Row>
						{:else}
							{#each plans as plan (plan.classroomCourseId)}
								<Table.Row>
									<Table.Cell>
										<div class="font-medium">{courseTitle(plan)}</div>
										<div class="text-xs text-muted-foreground">
											{plan.categoryCount} หมวด · {plan.itemCount} รายการย่อย
										</div>
									</Table.Cell>
									<Table.Cell>{plan.classroomName ?? '-'}</Table.Cell>
									<Table.Cell>{plan.instructorName ?? '-'}</Table.Cell>
									<Table.Cell class="text-right tabular-nums">{plan.totalScore}</Table.Cell>
									<Table.Cell>
										<div class="flex flex-wrap gap-1">
											{#if plan.inTimetableCount > 0}
												<Badge variant="secondary">ในตาราง {plan.inTimetableCount}</Badge>
											{/if}
											{#if plan.outsideTimetableCount > 0}
												<Badge variant="outline">นอกตาราง {plan.outsideTimetableCount}</Badge>
											{/if}
											{#if plan.inTimetableCount === 0 && plan.outsideTimetableCount === 0}
												<span class="text-sm text-muted-foreground">ไม่มีการสอบ</span>
											{/if}
										</div>
									</Table.Cell>
									<Table.Cell>
										<div class="flex flex-wrap items-center gap-2">
											<Badge variant={statusBadgeVariant(plan.status)}>
												{statusLabel(plan.status)}
											</Badge>
											{#if plan.hasUnallocatedCategories}
												<Badge variant="destructive">
													<AlertTriangle class="h-3 w-3" />
													คะแนนย่อยไม่ลงตัว
												</Badge>
											{/if}
										</div>
									</Table.Cell>
									<Table.Cell class="text-right">
										<Button
											variant="ghost"
											size="icon"
											disabled={!canManageAssessment}
											onclick={() => openPlanEditor(plan)}
											title="แก้โครงสร้างคะแนน"
										>
											<Pencil class="h-4 w-4" />
										</Button>
									</Table.Cell>
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		</div>
	{/if}
</PageShell>

<Dialog.Root bind:open={editorOpen}>
	<Dialog.Content class="flex max-h-[88vh] max-w-5xl flex-col overflow-hidden">
		<Dialog.Header>
			<Dialog.Title>โครงสร้างคะแนนรายวิชา</Dialog.Title>
			<Dialog.Description>
				{editingCourse
					? `${courseTitle(editingCourse)} · ${editingCourse.classroomName ?? ''}`
					: ''}
			</Dialog.Description>
		</Dialog.Header>

		{#if editorLoading}
			<div class="py-12 text-center text-muted-foreground">
				<Loader2 class="mx-auto mb-2 h-6 w-6 animate-spin" />
				กำลังโหลดโครงสร้างคะแนน
			</div>
		{:else}
			<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
				<div class="flex items-center justify-between gap-3">
					<div>
						<p class="text-sm text-muted-foreground">
							สถานะ: {editingPlan ? statusLabel(editingPlan.status) : '-'}
						</p>
					</div>
					<Button variant="outline" size="sm" onclick={addCategory}>
						<Plus class="mr-2 h-4 w-4" />
						เพิ่มหมวด
					</Button>
				</div>

				{#each editorCategories as category, categoryIndex (category.clientId)}
					<div class="rounded-lg border p-4">
						<div class="grid gap-3 lg:grid-cols-[1fr_120px_180px_auto]">
							<div class="space-y-2">
								<Label>หมวดคะแนน</Label>
								<Input bind:value={category.name} />
							</div>
							<div class="space-y-2">
								<Label>คะแนนเต็ม</Label>
								<Input type="number" min="0" step="0.5" bind:value={category.maxScore} />
							</div>
							<div class="space-y-2">
								<Label>รูปแบบ</Label>
								<Select.Root type="single" bind:value={category.examMode}>
									<Select.Trigger>{examModeLabel(category.examMode)}</Select.Trigger>
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
									onclick={() => removeCategory(categoryIndex)}
									title="ลบหมวด"
								>
									<Trash2 class="h-4 w-4" />
								</Button>
							</div>
						</div>

						<div class="mt-4 rounded-md bg-muted/40 p-3">
							<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
								<div class="text-sm text-muted-foreground">
									คะแนนย่อยรวม {categoryTotal(category)} / {Number(category.maxScore || 0)}
									<span class="ml-2 font-medium">
										{allocationStatusLabel(
											category.items.length === 0
												? Number(category.maxScore || 0) === 0
													? 'not_started'
													: 'complete'
												: Math.abs(categoryTotal(category) - Number(category.maxScore || 0)) <
													  0.0001
													? 'complete'
													: categoryTotal(category) < Number(category.maxScore || 0)
														? 'under_allocated'
														: 'over_allocated'
										)}
									</span>
								</div>
								<Button variant="outline" size="sm" onclick={() => addItem(categoryIndex)}>
									<Plus class="mr-2 h-4 w-4" />
									เพิ่มคะแนนย่อย
								</Button>
							</div>

							{#if category.items.length === 0}
								<p class="text-sm text-muted-foreground">
									ยังไม่แยกคะแนนย่อย ระบบจะถือว่าหมวดนี้เป็นรายการเดียวชั่วคราว
								</p>
							{:else}
								<div class="space-y-2">
									{#each category.items as item, itemIndex (item.clientId)}
										<div
											class="grid gap-2 rounded-md bg-background p-2 md:grid-cols-[1fr_110px_100px_auto]"
										>
											<Input bind:value={item.name} />
											<Input type="number" min="0" step="0.5" bind:value={item.maxScore} />
											<label class="flex items-center gap-2 text-sm">
												<Checkbox bind:checked={item.isActive} />
												ใช้งาน
											</label>
											<Button
												variant="ghost"
												size="icon"
												onclick={() => removeItem(categoryIndex, itemIndex)}
												title="ลบคะแนนย่อย"
											>
												<Trash2 class="h-4 w-4" />
											</Button>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}

		<Dialog.Footer class="gap-2">
			<Button
				variant="outline"
				onclick={() => (editorOpen = false)}
				disabled={saving || submitting}
			>
				ปิด
			</Button>
			<Button
				variant="outline"
				onclick={saveEditor}
				disabled={editorLoading || saving || submitting}
			>
				{#if saving}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{:else}
					<Save class="mr-2 h-4 w-4" />
				{/if}
				บันทึกร่าง
			</Button>
			<Button onclick={submitEditor} disabled={editorLoading || saving || submitting}>
				{#if submitting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{:else}
					<Send class="mr-2 h-4 w-4" />
				{/if}
				ส่งโครงสร้าง
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
