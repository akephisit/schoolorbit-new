<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listStudyPlans,
		createStudyPlan,
		updateStudyPlan,
		deleteStudyPlan,
		listStudyPlanVersions,
		createStudyPlanVersion,
		updateStudyPlanVersion,
		deleteStudyPlanVersion,
		listStudyPlanSubjects,
		addSubjectsToVersion,
		deleteStudyPlanSubject,
		lookupAcademicYears,
		lookupGradeLevels,
		listSubjects,
		type StudyPlan,
		type StudyPlanVersion,
		type StudyPlanSubject,
		type LookupItem,
		type Subject
	} from '$lib/api/academic';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import * as Table from '$lib/components/ui/table';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { GraduationCap, Plus, Pencil, Trash2, BookOpen, ListTodo } from 'lucide-svelte';

	let { data } = $props();

	// Data States
	let plans: StudyPlan[] = $state([]);
	let versions: StudyPlanVersion[] = $state([]);
	let planSubjects: StudyPlanSubject[] = $state([]);
	let academicYears: LookupItem[] = $state([]);
	let gradeLevels: LookupItem[] = $state([]);
	let subjects: Subject[] = $state([]);
	let loading = $state(true);

	// UI States
	let activeTab = $state('plans');
	let showPlanDialog = $state(false);
	let showVersionDialog = $state(false);
	let showSubjectsDialog = $state(false);
	let showDeleteDialog = $state(false);
	let submitting = $state(false);
	let deleteTarget: { type: 'plan' | 'version' | 'subject'; id: string; name: string } | null =
		$state(null);

	// Selected Items
	let selectedPlan: StudyPlan | null = $state(null);
	let selectedVersion: StudyPlanVersion | null = $state(null);

	// Form States
	let planForm = $state(getEmptyPlanForm());
	let versionForm = $state(getEmptyVersionForm());
	let selectedSubjects: string[] = $state([]);
	let selectedGrade = $state('');
	let selectedTerm = $state('1');

	// Derived: Filter grade levels based on selected version's study plan level_scope
	let filteredGradeLevels = $derived(
		gradeLevels.filter((grade) => {
			if (!selectedVersion) return true; // Show all if no version selected

			// Get the study plan's level_scope
			const plan = plans.find((p) => p.id === selectedVersion!.study_plan_id);
			if (!plan || !plan.level_scope || plan.level_scope === 'all') return true;

			// Check if grade level matches the plan's scope
			const levelType = grade.level_type; // 'kindergarten', 'primary', 'secondary'
			return levelType === plan.level_scope;
		})
	);

	// Derived: Filter subjects based on selected grade
	let filteredSubjects = $derived(
		subjects.filter((subject) => {
			if (!selectedGrade) return false; // Don't show any subjects if no grade selected

			// Filter by Term
			if (selectedTerm && subject.term && subject.term !== selectedTerm) {
				return false;
			}

			// Get the grade level info
			const grade = gradeLevels.find((g) => g.id === selectedGrade);
			if (!grade) return false;

			// 1. Check specific grade_level_ids (Specific Scope)
			if (subject.grade_level_ids && subject.grade_level_ids.length > 0) {
				return subject.grade_level_ids.includes(selectedGrade);
			}

			// 2. If subject has level_scope, check if it matches
			if (subject.level_scope) {
				// level_scope can be 'ALL', 'M1', 'M2', 'P1', etc.
				if (subject.level_scope.toUpperCase() === 'ALL') return true;

				// Match by shortname (e.g., 'ม.1' matches 'M1')
				const gradeShortName = grade.code || ''; // e.g., 'ม.1'
				const scopeUpper = subject.level_scope.toUpperCase(); // e.g., 'M1'

				// Convert 'ม.1' to 'M1', 'ป.1' to 'P1', etc.
				const normalizedGrade = gradeShortName
					.replace('ม.', 'M')
					.replace('ป.', 'P')
					.replace('อ.', 'K');

				return scopeUpper === normalizedGrade;
			}

			return true; // If no level_scope, show for all grades
		})
	);

	function getEmptyPlanForm(): {
		id: string;
		code: string;
		name_th: string;
		name_en?: string;
		description?: string;
		level_scope?: string;
	} {
		return {
			id: '',
			code: '',
			name_th: '',
			name_en: '',
			description: '',
			level_scope: 'all'
		};
	}

	function getEmptyVersionForm(): {
		id: string;
		study_plan_id: string;
		version_name: string;
		start_academic_year_id: string;
		end_academic_year_id?: string;
		description?: string;
	} {
		return {
			id: '',
			study_plan_id: '',
			version_name: '',
			start_academic_year_id: '',
			end_academic_year_id: '',
			description: ''
		};
	}

	async function initData() {
		try {
			loading = true;
			const [plansRes, yearsRes, levelsRes, subjectsRes] = await Promise.all([
				listStudyPlans(),
				lookupAcademicYears(false),
				lookupGradeLevels({}),
				listSubjects({ active_only: true })
			]);

			plans = plansRes.data;
			academicYears = yearsRes.data;
			gradeLevels = levelsRes.data;
			subjects = subjectsRes.data;
		} catch (e) {
			alert('เกิดข้อผิดพลาด: ' + (e instanceof Error ? e.message : ''));
		} finally {
			loading = false;
		}
	}

	async function loadVersions(planId: string) {
		try {
			const res = await listStudyPlanVersions({ study_plan_id: planId });
			versions = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	async function loadPlanSubjects(versionId: string, gradeId?: string, term?: string) {
		try {
			const res = await listStudyPlanSubjects({
				study_plan_version_id: versionId,
				grade_level_id: gradeId,
				term: term
			});
			planSubjects = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	// Plan Handlers
	function handleOpenCreatePlan() {
		planForm = getEmptyPlanForm();
		showPlanDialog = true;
	}

	function handleOpenEditPlan(plan: StudyPlan) {
		planForm = { ...plan };
		showPlanDialog = true;
	}

	async function handleSubmitPlan() {
		if (!planForm.code || !planForm.name_th) {
			alert('กรุณากรอกข้อมูลให้ครบถ้วน');
			return;
		}

		submitting = true;
		try {
			if (planForm.id) {
				await updateStudyPlan(planForm.id, planForm);
			} else {
				await createStudyPlan(planForm);
			}
			showPlanDialog = false;
			await initData();
		} catch (e) {
			alert('บันทึกไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			submitting = false;
		}
	}

	// Version Handlers
	function handleOpenCreateVersion(plan: StudyPlan) {
		versionForm = {
			...getEmptyVersionForm(),
			study_plan_id: plan.id
		};
		selectedPlan = plan;
		showVersionDialog = true;
	}

	function handleOpenEditVersion(version: StudyPlanVersion) {
		versionForm = { ...version };
		showVersionDialog = true;
	}

	async function handleSubmitVersion() {
		if (!versionForm.version_name || !versionForm.start_academic_year_id) {
			alert('กรุณากรอกข้อมูลให้ครบถ้วน');
			return;
		}

		submitting = true;
		try {
			if (versionForm.id) {
				await updateStudyPlanVersion(versionForm.id, versionForm);
			} else {
				await createStudyPlanVersion(versionForm);
			}
			showVersionDialog = false;
			if (selectedPlan) {
				await loadVersions(selectedPlan.id);
			}
		} catch (e) {
			alert('บันทึกไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			submitting = false;
		}
	}

	// Subject Handlers
	function handleOpenAddSubjects(version: StudyPlanVersion) {
		selectedVersion = version;
		selectedSubjects = [];
		selectedTerm = '1';

		// Calculate valid grade levels based on plan scope
		const plan = plans.find((p) => p.id === version.study_plan_id);
		let validGrades = gradeLevels;
		if (plan && plan.level_scope && plan.level_scope !== 'all') {
			validGrades = gradeLevels.filter((g) => g.level_type === plan.level_scope);
		}

		// Set first valid grade as default
		selectedGrade = validGrades.length > 0 ? validGrades[0].id : '';

		showSubjectsDialog = true;
	}

	async function handleSubmitSubjects() {
		if (!selectedVersion || selectedSubjects.length === 0 || !selectedGrade) {
			alert('กรุณาเลือกรายวิชาและระดับชั้น');
			return;
		}

		submitting = true;
		try {
			const subjectsToAdd = selectedSubjects.map((subjectId, index) => ({
				subject_id: subjectId,
				grade_level_id: selectedGrade,
				term: selectedTerm,
				is_required: true,
				display_order: index
			}));

			await addSubjectsToVersion(selectedVersion.id, subjectsToAdd);
			showSubjectsDialog = false;
			await loadPlanSubjects(selectedVersion.id);
		} catch (e) {
			alert('เพิ่มรายวิชาไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			submitting = false;
		}
	}

	// Delete Handlers
	function handleOpenDelete(type: 'plan' | 'version' | 'subject', id: string, name: string) {
		deleteTarget = { type, id, name };
		showDeleteDialog = true;
	}

	async function handleConfirmDelete() {
		if (!deleteTarget) return;

		submitting = true;
		try {
			if (deleteTarget.type === 'plan') {
				await deleteStudyPlan(deleteTarget.id);
				await initData();
			} else if (deleteTarget.type === 'version') {
				await deleteStudyPlanVersion(deleteTarget.id);
				if (selectedPlan) await loadVersions(selectedPlan.id);
			} else {
				await deleteStudyPlanSubject(deleteTarget.id);
				if (selectedVersion) await loadPlanSubjects(selectedVersion.id);
			}
			showDeleteDialog = false;
		} catch (e) {
			alert('ลบไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			submitting = false;
		}
	}

	onMount(() => {
		initData();
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<GraduationCap class="w-8 h-8" />
				หลักสูตรสถานศึกษา
			</h1>
			<p class="text-muted-foreground mt-1">จัดการหลักสูตรและเวอร์ชันของหลักสูตร</p>
		</div>
	</div>

	<!-- Tabs -->
	<Tabs.Root bind:value={activeTab}>
		<Tabs.List class="grid w-full grid-cols-3">
			<Tabs.Trigger value="plans">แผนการเรียน</Tabs.Trigger>
			<Tabs.Trigger value="versions">เวอร์ชัน</Tabs.Trigger>
			<Tabs.Trigger value="subjects">รายวิชา</Tabs.Trigger>
		</Tabs.List>

		<!-- Plans Tab -->
		<Tabs.Content value="plans" class="space-y-4">
			<div class="flex justify-end">
				<Button onclick={handleOpenCreatePlan} class="flex items-center gap-2">
					<Plus class="w-4 h-4" />
					สร้างแผนใหม่
				</Button>
			</div>

			<div class="bg-card border border-border rounded-lg overflow-hidden">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head>รหัส</Table.Head>
							<Table.Head>ชื่อแผน</Table.Head>
							<Table.Head>ระดับ</Table.Head>
							<Table.Head class="w-[150px]">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loading}
							<Table.Row>
								<Table.Cell colspan={4} class="text-center h-24">กำลังโหลด...</Table.Cell>
							</Table.Row>
						{:else if plans.length === 0}
							<Table.Row>
								<Table.Cell colspan={4} class="text-center h-24 text-muted-foreground">
									ไม่พบข้อมูล
								</Table.Cell>
							</Table.Row>
						{:else}
							{#each plans as plan (plan.id)}
								<Table.Row>
									<Table.Cell class="font-medium">{plan.code}</Table.Cell>
									<Table.Cell>
										<div class="font-medium">{plan.name_th}</div>
										{#if plan.name_en}
											<div class="text-xs text-muted-foreground">{plan.name_en}</div>
										{/if}
									</Table.Cell>
									<Table.Cell>
										<Badge variant="secondary">{plan.level_scope || 'ทุกระดับ'}</Badge>
									</Table.Cell>
									<Table.Cell>
										<div class="flex gap-1">
											<Button
												onclick={() => {
													selectedPlan = plan;
													selectedVersion = null;
													loadVersions(plan.id);
													activeTab = 'versions';
												}}
												variant="ghost"
												size="sm"
											>
												<BookOpen class="w-4 h-4 mr-1" />
												เวอร์ชัน
											</Button>
											<Button
												onclick={() => handleOpenEditPlan(plan)}
												variant="ghost"
												size="icon"
												class="h-8 w-8"
											>
												<Pencil class="w-4 h-4" />
											</Button>
											<Button
												onclick={() => handleOpenDelete('plan', plan.id, plan.name_th)}
												variant="ghost"
												size="icon"
												class="h-8 w-8 text-destructive"
											>
												<Trash2 class="w-4 h-4" />
											</Button>
										</div>
									</Table.Cell>
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		</Tabs.Content>

		<!-- Versions Tab -->
		<Tabs.Content value="versions" class="space-y-4">
			{#if selectedPlan}
				<div class="bg-muted/50 p-4 rounded-lg">
					<div class="font-medium">แผนการเรียน: {selectedPlan.name_th}</div>
					<Button
						onclick={() => selectedPlan && handleOpenCreateVersion(selectedPlan)}
						variant="outline"
						size="sm"
						class="mt-2"
					>
						<Plus class="w-4 h-4 mr-1" />
						สร้างเวอร์ชันใหม่
					</Button>
				</div>

				<div class="bg-card border border-border rounded-lg overflow-hidden">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>ชื่อเวอร์ชัน</Table.Head>
								<Table.Head>ปีการศึกษาเริ่มต้น</Table.Head>
								<Table.Head>สถานะ</Table.Head>
								<Table.Head class="w-[150px]">จัดการ</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if versions.length === 0}
								<Table.Row>
									<Table.Cell colspan={4} class="text-center h-24 text-muted-foreground">
										ยังไม่มีเวอร์ชัน
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each versions as version (version.id)}
									<Table.Row>
										<Table.Cell class="font-medium">{version.version_name}</Table.Cell>
										<Table.Cell>{version.start_year_name || '-'}</Table.Cell>
										<Table.Cell>
											{#if version.is_active}
												<Badge>ใช้งาน</Badge>
											{:else}
												<Badge variant="secondary">ปิดใช้งาน</Badge>
											{/if}
										</Table.Cell>
										<Table.Cell>
											<div class="flex gap-1">
												<Button
													onclick={() => {
														selectedVersion = version;
														loadPlanSubjects(version.id);
														activeTab = 'subjects';
													}}
													variant="ghost"
													size="sm"
												>
													<ListTodo class="w-4 h-4 mr-1" />
													รายวิชา
												</Button>
												<Button
													onclick={() => handleOpenEditVersion(version)}
													variant="ghost"
													size="icon"
													class="h-8 w-8"
												>
													<Pencil class="w-4 h-4" />
												</Button>
												<Button
													onclick={() =>
														handleOpenDelete('version', version.id, version.version_name)}
													variant="ghost"
													size="icon"
													class="h-8 w-8 text-destructive"
												>
													<Trash2 class="w-4 h-4" />
												</Button>
											</div>
										</Table.Cell>
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</div>
			{:else}
				<div class="text-center text-muted-foreground p-8">
					กรุณาเลือกแผนการเรียนจากแท็บ "แผนการเรียน"
				</div>
			{/if}
		</Tabs.Content>

		<!-- Subjects Tab -->
		<Tabs.Content value="subjects" class="space-y-4">
			{#if selectedVersion}
				<div class="bg-muted/50 p-4 rounded-lg">
					<div class="font-medium">
						เวอร์ชัน: {selectedVersion.version_name}
						{#if selectedVersion.study_plan_name_th}
							({selectedVersion.study_plan_name_th})
						{/if}
					</div>
					<div class="flex gap-2 mt-2">
						<Button
							onclick={() => selectedVersion && handleOpenAddSubjects(selectedVersion)}
							variant="outline"
							size="sm"
						>
							<Plus class="w-4 h-4 mr-1" />
							เพิ่มรายวิชา
						</Button>
					</div>
				</div>

				<div class="bg-card border border-border rounded-lg overflow-hidden">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>รหัสวิชา</Table.Head>
								<Table.Head>ชื่อวิชา</Table.Head>
								<Table.Head>ระดับชั้น</Table.Head>
								<Table.Head>เทอม</Table.Head>
								<Table.Head class="w-[80px]">จัดการ</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if planSubjects.length === 0}
								<Table.Row>
									<Table.Cell colspan={5} class="text-center h-24 text-muted-foreground">
										ยังไม่มีรายวิชา
									</Table.Cell>
								</Table.Row>
							{:else}
								{#each planSubjects as subject (subject.id)}
									<Table.Row>
										<Table.Cell class="font-medium">{subject.subject_code}</Table.Cell>
										<Table.Cell>{subject.subject_name_th || '-'}</Table.Cell>
										<Table.Cell>{subject.grade_level_name || '-'}</Table.Cell>
										<Table.Cell>ภาคเรียนที่ {subject.term}</Table.Cell>
										<Table.Cell>
											<Button
												onclick={() =>
													handleOpenDelete(
														'subject',
														subject.id,
														subject.subject_name_th || subject.subject_code
													)}
												variant="ghost"
												size="icon"
												class="h-8 w-8 text-destructive"
											>
												<Trash2 class="w-4 h-4" />
											</Button>
										</Table.Cell>
									</Table.Row>
								{/each}
							{/if}
						</Table.Body>
					</Table.Root>
				</div>
			{:else}
				<div class="text-center text-muted-foreground p-8">
					กรุณาเลือกเวอร์ชันจากแท็บ "เวอร์ชัน"
				</div>
			{/if}
		</Tabs.Content>
	</Tabs.Root>
</div>

<!-- Plan Dialog -->
<Dialog bind:open={showPlanDialog}>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>{planForm.id ? 'แก้ไข' : 'สร้าง'}แผนการเรียน</DialogTitle>
			<DialogDescription>กรอกข้อมูลแผนการเรียน</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="space-y-2">
				<Label>รหัส <span class="text-destructive">*</span></Label>
				<Input bind:value={planForm.code} placeholder="PLAN001" />
			</div>

			<div class="space-y-2">
				<Label>ชื่อแผน (ภาษาไทย) <span class="text-destructive">*</span></Label>
				<Input bind:value={planForm.name_th} placeholder="แผนการเรียนวิทย์-คณิต" />
			</div>

			<div class="space-y-2">
				<Label>ชื่อแผน (English)</Label>
				<Input bind:value={planForm.name_en} placeholder="Science-Mathematics Program" />
			</div>

			<div class="space-y-2">
				<Label>ระดับชั้น</Label>
				<Select.Root type="single" bind:value={planForm.level_scope}>
					<Select.Trigger>
						{planForm.level_scope === 'kindergarten'
							? 'อนุบาล'
							: planForm.level_scope === 'primary'
								? 'ประถม'
								: planForm.level_scope === 'secondary'
									? 'มัธยม'
									: 'ทุกระดับ'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">ทุกระดับ</Select.Item>
						<Select.Item value="kindergarten">อนุบาล</Select.Item>
						<Select.Item value="primary">ประถม</Select.Item>
						<Select.Item value="secondary">มัธยม</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={planForm.description} placeholder="คำอธิบายแผนการเรียน" />
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showPlanDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSubmitPlan} disabled={submitting}>
				{submitting ? 'กำลังบันทึก...' : 'บันทึก'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Version Dialog -->
<Dialog bind:open={showVersionDialog}>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>{versionForm.id ? 'แก้ไข' : 'สร้าง'}เวอร์ชันหลักสูตร</DialogTitle>
			<DialogDescription>กรอกข้อมูลเวอร์ชัน</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="space-y-2">
				<Label>ชื่อเวอร์ชัน <span class="text-destructive">*</span></Label>
				<Input bind:value={versionForm.version_name} placeholder="เช่น v2568" />
			</div>

			<div class="space-y-2">
				<Label>ปีการศึกษาเริ่มต้น <span class="text-destructive">*</span></Label>
				<Select.Root type="single" bind:value={versionForm.start_academic_year_id}>
					<Select.Trigger>
						{academicYears.find((y) => y.id === versionForm.start_academic_year_id)?.name ||
							'เลือกปีการศึกษา'}
					</Select.Trigger>
					<Select.Content>
						{#each academicYears as year}
							<Select.Item value={year.id}>{year.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label>ปีการศึกษาสิ้นสุด (ถ้ามี)</Label>
				<Select.Root type="single" bind:value={versionForm.end_academic_year_id}>
					<Select.Trigger>
						{academicYears.find((y) => y.id === versionForm.end_academic_year_id)?.name ||
							'ไม่ระบุ'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="">ไม่ระบุ</Select.Item>
						{#each academicYears as year}
							<Select.Item value={year.id}>{year.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label>คำอธิบาย</Label>
				<Textarea bind:value={versionForm.description} placeholder="คำอธิบายเวอร์ชัน" />
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showVersionDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSubmitVersion} disabled={submitting}>
				{submitting ? 'กำลังบันทึก...' : 'บันทึก'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Add Subjects Dialog -->
<Dialog bind:open={showSubjectsDialog}>
	<DialogContent class="sm:max-w-[600px] max-h-[80vh] overflow-y-auto">
		<DialogHeader>
			<DialogTitle>เพิ่มรายวิชา</DialogTitle>
			<DialogDescription>เลือกรายวิชาที่ต้องการเพิ่มเข้าหลักสูตร</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label>ระดับชั้น <span class="text-destructive">*</span></Label>
					{#if filteredGradeLevels.length === 0}
						<div
							class="text-sm text-destructive p-2 border border-destructive/20 bg-destructive/10 rounded"
						>
							ไม่พบระดับชั้นที่ตรงกับแผนการเรียนนี้ (โปรดตรวจสอบ "ขอบเขตระดับชั้น"
							ในการตั้งค่าแผนการเรียน หรือเพิ่มระดับชั้นในระบบ)
						</div>
					{:else}
						<Select.Root type="single" bind:value={selectedGrade}>
							<Select.Trigger>
								{filteredGradeLevels.find((g) => g.id === selectedGrade)?.name || 'เลือกระดับชั้น'}
							</Select.Trigger>
							<Select.Content>
								{#each filteredGradeLevels as level}
									<Select.Item value={level.id}>{level.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					{/if}
				</div>

				<div class="space-y-2">
					<Label>ภาคเรียน <span class="text-destructive">*</span></Label>
					<Select.Root type="single" bind:value={selectedTerm}>
						<Select.Trigger>ภาคเรียนที่ {selectedTerm}</Select.Trigger>
						<Select.Content>
							<Select.Item value="1">ภาคเรียนที่ 1</Select.Item>
							<Select.Item value="2">ภาคเรียนที่ 2</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<div class="space-y-2">
				<Label>เลือกรายวิชา</Label>
				<div class="border rounded-md p-2 max-h-[300px] overflow-y-auto space-y-1">
					{#if filteredSubjects.length === 0}
						<div class="text-center text-muted-foreground p-4">
							{#if selectedGrade}
								ไม่พบรายวิชาสำหรับระดับชั้นนี้
							{:else}
								กรุณาเลือกระดับชั้น
							{/if}
						</div>
					{:else}
						{#each [...filteredSubjects].sort((a, b) => {
							const aAdded = planSubjects.some((ps) => ps.subject_id === a.id && ps.grade_level_id === selectedGrade);
							const bAdded = planSubjects.some((ps) => ps.subject_id === b.id && ps.grade_level_id === selectedGrade);
							// Sort: Not added first (0), Added last (1)
							return Number(aAdded) - Number(bAdded) || a.code.localeCompare(b.code);
						}) as subject}
							{@const isAdded = planSubjects.some(
								(ps) => ps.subject_id === subject.id && ps.grade_level_id === selectedGrade
							)}
							<label
								class="flex items-center gap-2 p-2 hover:bg-muted rounded cursor-pointer transition-colors"
								class:opacity-50={isAdded}
								class:bg-muted={isAdded}
							>
								<Checkbox
									checked={isAdded || selectedSubjects.includes(subject.id)}
									disabled={isAdded}
									onCheckedChange={(v) => {
										const checked = v === true; // Handle boolean | 'indeterminate'
										if (checked) {
											selectedSubjects = [...selectedSubjects, subject.id];
										} else {
											selectedSubjects = selectedSubjects.filter((id) => id !== subject.id);
										}
									}}
								/>
								<div class="flex-1 flex items-center justify-between">
									<div class="flex flex-col">
										<div class="flex items-center gap-2">
											<span class="font-medium text-sm">{subject.code}</span>
											{#if subject.term}
												<Badge
													variant="outline"
													class="text-[10px] px-1 h-4 bg-blue-50 text-blue-700 border-blue-200"
												>
													เทอม {subject.term}
												</Badge>
											{/if}
											{#if isAdded}
												<Badge variant="secondary" class="text-[10px] px-1 h-4">เลือกแล้ว</Badge>
											{/if}
										</div>
										<span class="text-xs text-muted-foreground">{subject.name_th}</span>
									</div>

									{#if subject.level_scope && subject.level_scope.toUpperCase() !== 'ALL'}
										<Badge variant="outline" class="text-[10px] h-5">{subject.level_scope}</Badge>
									{/if}
								</div>
							</label>
						{/each}
					{/if}
				</div>
				<p class="text-xs text-muted-foreground">
					เลือกแล้ว: {selectedSubjects.length} วิชา
					{#if filteredSubjects.length > 0}
						(จาก {filteredSubjects.length} วิชาที่แสดง)
					{/if}
				</p>
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showSubjectsDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSubmitSubjects} disabled={submitting || selectedSubjects.length === 0}>
				{submitting ? 'กำลังเพิ่ม...' : `เพิ่มรายวิชา (${selectedSubjects.length})`}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Delete Confirmation Dialog -->
<Dialog bind:open={showDeleteDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการลบ</DialogTitle>
			<DialogDescription>
				คุณแน่ใจหรือไม่ที่จะลบ <strong>{deleteTarget?.name}</strong>?
				การกระทำนี้ไม่สามารถย้อนกลับได้
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleConfirmDelete} disabled={submitting}>
				{submitting ? 'กำลังลบ...' : 'ลบ'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
