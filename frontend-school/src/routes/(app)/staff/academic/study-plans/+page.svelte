<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
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
		listSubjectGroups,
		listPlanActivities,
		addPlanActivity,
		updatePlanActivity,
		deletePlanActivity,
		listActivityCatalog,
		type StudyPlan,
		type StudyPlanVersion,
		type StudyPlanSubject,
		type StudyPlanVersionActivity,
		type ActivityCatalog,
		type LookupItem,
		type Subject,
		type SubjectGroup
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
	let subjectGroups: SubjectGroup[] = $state([]);
	let loading = $state(true);

	// UI States
	let activeTab = $state('plans');
	let showPlanDialog = $state(false);
	let showVersionDialog = $state(false);
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

	function getEmptyPlanForm(): {
		id: string;
		code: string;
		name_th: string;
		name_en?: string;
		description?: string;
		level_scope?: string;
		grade_level_ids: string[];
	} {
		return {
			id: '',
			code: '',
			name_th: '',
			name_en: '',
			description: '',
			level_scope: 'all',
			grade_level_ids: []
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
			const [plansRes, yearsRes, levelsRes, subjectsRes, groupsRes] = await Promise.all([
				listStudyPlans(),
				lookupAcademicYears(false),
				lookupGradeLevels({}),
				listSubjects({ active_only: true }),
				listSubjectGroups()
			]);

			plans = plansRes.data;
			academicYears = yearsRes.data;
			gradeLevels = levelsRes.data;
			subjects = subjectsRes.data;
			subjectGroups = groupsRes.data ?? [];
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
		planForm = {
			...getEmptyPlanForm(),
			...plan,
			grade_level_ids: plan.grade_level_ids ?? []
		};
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

	// ==========================================
	// Plan Activities (template)
	// ==========================================
	let planActivities = $state<StudyPlanVersionActivity[]>([]);
	let loadingPlanActivities = $state(false);
	let showPlanActivityDialog = $state(false);
	let editingPlanActivity = $state<StudyPlanVersionActivity | null>(null);
	let activityCatalog = $state<ActivityCatalog[]>([]);
	let paCatalogId = $state('');
	let paGradeLevelIds = $state<string[]>([]);
	let paIsRequired = $state(true);

	async function loadActivityCatalog() {
		try {
			const res = await listActivityCatalog();
			activityCatalog = res.data ?? [];
		} catch {
			activityCatalog = [];
		}
	}

	const PA_TYPE_LABELS: Record<string, string> = {
		scout: 'ลูกเสือ / เนตรนารี / ยุวกาชาด',
		club: 'ชุมนุม',
		guidance: 'แนะแนว',
		social: 'กิจกรรมเพื่อสังคม',
		other: 'อื่น ๆ'
	};

	async function loadPlanActivitiesForVersion(versionId: string) {
		if (!versionId) {
			planActivities = [];
			return;
		}
		loadingPlanActivities = true;
		try {
			const res = await listPlanActivities(versionId);
			planActivities = res.data ?? [];
		} catch {
			planActivities = [];
		} finally {
			loadingPlanActivities = false;
		}
	}

	function openEditPlanActivity(pa: StudyPlanVersionActivity) {
		editingPlanActivity = pa;
		paCatalogId = pa.activity_catalog_id;
		paGradeLevelIds = pa.allowed_grade_level_ids ?? [];
		paIsRequired = pa.is_required;
		showPlanActivityDialog = true;
		loadActivityCatalog();
	}

	async function handleSavePlanActivity() {
		const versionId = selectedVersion?.id;
		if (!versionId) {
			toast.error('กรุณาเลือก version');
			return;
		}

		if (!paCatalogId) {
			toast.error('กรุณาเลือกกิจกรรมจากคลัง');
			return;
		}

		const payload = {
			activity_catalog_id: paCatalogId,
			allowed_grade_level_ids: paGradeLevelIds.length > 0 ? paGradeLevelIds : undefined,
			is_required: paIsRequired
		};

		try {
			if (editingPlanActivity) {
				await updatePlanActivity(editingPlanActivity.id, payload);
				toast.success('บันทึกแล้ว');
			} else {
				await addPlanActivity(versionId, payload);
				toast.success('เพิ่มกิจกรรมแล้ว');
			}
			showPlanActivityDialog = false;
			await loadPlanActivitiesForVersion(versionId);
		} catch {
			toast.error('บันทึกไม่สำเร็จ');
		}
	}

	async function handleDeletePlanActivity(pa: StudyPlanVersionActivity) {
		if (!confirm(`ลบกิจกรรม "${pa.catalog_name ?? ''}"?`)) return;
		try {
			await deletePlanActivity(pa.id);
			toast.success('ลบแล้ว');
			const versionId = selectedVersion?.id;
			if (versionId) await loadPlanActivitiesForVersion(versionId);
		} catch {
			toast.error('ลบไม่สำเร็จ');
		}
	}

	// Reload plan activities whenever selectedVersion changes
	$effect(() => {
		if (selectedVersion?.id) {
			loadPlanActivitiesForVersion(selectedVersion.id);
		} else {
			planActivities = [];
		}
	});

	// ==========================================
	// Unified "Add to Plan" dialog (วิชา + กิจกรรม, multi-select)
	// ==========================================
	let showAddDialog = $state(false);
	let addGradeLevelIds = $state<string[]>([]); // multi-select grade (applies to all picks)
	let addTerm = $state('1');
	let addIsRequired = $state(true);

	// Subject picker state
	let addGroupFilter = $state(''); // group filter (optional)
	let selectedSubjectIds = $state<Set<string>>(new Set()); // checked subjects

	// Activity picker state
	let selectedCatalogIds = $state<Set<string>>(new Set()); // checked activities

	function toggleSubjectPick(id: string) {
		const next = new Set(selectedSubjectIds);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selectedSubjectIds = next;
	}

	function toggleCatalogPick(id: string) {
		const next = new Set(selectedCatalogIds);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selectedCatalogIds = next;
	}

	// Derived: valid grade levels for current version (prefer plan.grade_level_ids, fallback to level_scope)
	let unifiedGradeLevels = $derived.by(() => {
		if (!selectedVersion) return gradeLevels;
		const plan = plans.find((p) => p.id === selectedVersion!.study_plan_id);
		if (!plan) return gradeLevels;

		// New: if grade_level_ids is set, use those
		if (plan.grade_level_ids && plan.grade_level_ids.length > 0) {
			return gradeLevels.filter((g) => plan.grade_level_ids!.includes(g.id));
		}

		// Fallback: legacy level_scope
		if (!plan.level_scope || plan.level_scope === 'all') return gradeLevels;
		return gradeLevels.filter((g) => g.level_type === plan.level_scope);
	});

	// Filter subjects by group (uses the already-loaded `subjects` state from initData)
	let filteredSubjectsInDialog = $derived.by(() => {
		if (!addGroupFilter) return subjects;
		return subjects.filter((s) => s.group_id === addGroupFilter);
	});

	async function loadAllSubjects() {
		try {
			const res = await listSubjects({ active_only: true, latest_only: true });
			subjects = res.data ?? [];
		} catch {
			// keep existing
		}
	}

	function openAddDialog() {
		addGradeLevelIds = [];
		addTerm = '1';
		addIsRequired = true;
		addGroupFilter = '';
		selectedSubjectIds = new Set();
		selectedCatalogIds = new Set();
		showAddDialog = true;

		// Ensure catalogs are loaded
		if (activityCatalog.length === 0) loadActivityCatalog();
		if (subjects.length === 0) loadAllSubjects();
	}

	async function handleAddDialogSave() {
		if (!selectedVersion?.id) {
			toast.error('กรุณาเลือก version');
			return;
		}
		if (addGradeLevelIds.length === 0) {
			toast.error('กรุณาเลือกระดับชั้น');
			return;
		}
		if (selectedSubjectIds.size === 0 && selectedCatalogIds.size === 0) {
			toast.error('กรุณาเลือกวิชาหรือกิจกรรมอย่างน้อย 1 รายการ');
			return;
		}

		try {
			// Subjects: addSubjectsToVersion expects array of { subject_id, grade_level_id, term, is_required }
			if (selectedSubjectIds.size > 0) {
				const rows: {
					subject_id: string;
					grade_level_id: string;
					term: string;
					is_required?: boolean;
				}[] = [];
				for (const sid of selectedSubjectIds) {
					for (const gid of addGradeLevelIds) {
						rows.push({
							subject_id: sid,
							grade_level_id: gid,
							term: addTerm,
							is_required: addIsRequired
						});
					}
				}
				await addSubjectsToVersion(selectedVersion.id, rows);
			}

			// Activities: addPlanActivity once per catalog item
			if (selectedCatalogIds.size > 0) {
				for (const cid of selectedCatalogIds) {
					await addPlanActivity(selectedVersion.id, {
						activity_catalog_id: cid,
						allowed_grade_level_ids: addGradeLevelIds,
						is_required: addIsRequired
					});
				}
			}

			toast.success('เพิ่มเข้าหลักสูตรแล้ว');
			showAddDialog = false;

			// Reload
			await loadPlanSubjects(selectedVersion.id);
			await loadPlanActivitiesForVersion(selectedVersion.id);
		} catch {
			toast.error('บันทึกไม่สำเร็จ');
		}
	}

	// ==========================================
	// Derived state: integrated 2-column term view
	// ==========================================
	function termKey(t: string | null | undefined): '1' | '2' | 'other' {
		if (t === '1' || t === '2') return t;
		return 'other';
	}

	let subjectsByTermType = $derived.by(() => {
		const grouped: Record<'1' | '2', Record<'BASIC' | 'ADDITIONAL', StudyPlanSubject[]>> = {
			'1': { BASIC: [], ADDITIONAL: [] },
			'2': { BASIC: [], ADDITIONAL: [] }
		};
		for (const s of planSubjects) {
			const tk = termKey(s.term);
			if (tk === 'other') continue;
			const st = (s.subject_type ?? 'BASIC').toUpperCase();
			if (st === 'BASIC') grouped[tk].BASIC.push(s);
			else if (st === 'ADDITIONAL') grouped[tk].ADDITIONAL.push(s);
			else grouped[tk].BASIC.push(s); // fallback
		}
		return grouped;
	});

	let activitiesByTerm = $derived.by(() => {
		const grouped: Record<'1' | '2', StudyPlanVersionActivity[]> = { '1': [], '2': [] };
		for (const a of planActivities) {
			const at = a.catalog_term;
			if (at === null || at === undefined || at === '') {
				grouped['1'].push(a);
				grouped['2'].push(a);
			} else if (at === '1' || at === '2') {
				grouped[at].push(a);
			}
		}
		return grouped;
	});

	function sectionTotals(items: StudyPlanSubject[]) {
		const credits = items.reduce((s, x) => s + (x.subject_credit ?? 0), 0);
		const hours = items.reduce((s, x) => s + (x.subject_hours ?? 0), 0);
		return { credits, hours };
	}

	function activityTotals(items: StudyPlanVersionActivity[]) {
		const periods = items.reduce((s, x) => s + (x.catalog_periods_per_week ?? 0), 0);
		return { periods };
	}

	function handleDeletePlanSubject(s: StudyPlanSubject) {
		handleOpenDelete('subject', s.id, s.subject_name_th || s.subject_code);
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
			<Tabs.Trigger value="detail">รายละเอียดหลักสูตร</Tabs.Trigger>
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
										{#if plan.grade_level_ids && plan.grade_level_ids.length > 0}
											<Badge variant="secondary" class="text-[10px]">
												{plan.grade_level_ids.length} ระดับ: {plan.grade_level_ids
													.map(
														(id) =>
															gradeLevels.find((g) => g.id === id)?.short_name ?? ''
													)
													.filter(Boolean)
													.join(', ')}
											</Badge>
										{:else}
											<Badge variant="secondary" class="text-[10px]">ทุกระดับ</Badge>
										{/if}
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
														activeTab = 'detail';
													}}
													variant="ghost"
													size="sm"
												>
													<ListTodo class="w-4 h-4 mr-1" />
													รายละเอียด
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

		<!-- Detail Tab: integrated 2-column view by term -->
		<Tabs.Content value="detail" class="space-y-4">
			{#if selectedVersion}
				<div class="bg-muted/50 p-4 rounded-lg flex items-center justify-between flex-wrap gap-2">
					<div class="font-medium">
						เวอร์ชัน: {selectedVersion.version_name}
						{#if selectedVersion.study_plan_name_th}
							({selectedVersion.study_plan_name_th})
						{/if}
					</div>
					<Button onclick={openAddDialog} size="sm">
						<Plus class="w-4 h-4 mr-1" />
						เพิ่มเข้าหลักสูตร
					</Button>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
					{#each ['1', '2'] as term (term)}
						{@const tterm = term as '1' | '2'}
						{@const basicList = subjectsByTermType[tterm].BASIC}
						{@const additionalList = subjectsByTermType[tterm].ADDITIONAL}
						{@const actList = activitiesByTerm[tterm]}
						{@const tb = sectionTotals(basicList)}
						{@const ta = sectionTotals(additionalList)}
						{@const tact = activityTotals(actList)}
						<div class="border rounded-lg p-4 space-y-4 bg-card">
							<h2 class="text-lg font-bold border-b pb-2">ภาคเรียนที่ {term}</h2>

							<!-- วิชาพื้นฐาน -->
							<section>
								<h3 class="font-semibold flex items-center gap-1 mb-2 text-sm">
									<BookOpen class="w-4 h-4" /> วิชาพื้นฐาน
								</h3>
								{#if basicList.length === 0}
									<p class="text-xs text-muted-foreground italic">—</p>
								{:else}
									<div class="divide-y border rounded">
										{#each basicList as s (s.id)}
											<div class="px-3 py-2 flex items-center gap-2">
												<div class="flex-1 min-w-0">
													<div class="font-medium text-sm">
														{s.subject_code}
														{s.subject_name_th ?? ''}
														{#if s.grade_level_name}
															<span class="text-xs text-muted-foreground"
																>· {s.grade_level_name}</span
															>
														{/if}
													</div>
													<div class="text-xs text-muted-foreground">
														{(s.subject_credit ?? 0).toFixed(1)} นก · {s.subject_hours ?? 0} ชม
													</div>
												</div>
												<Button
													variant="ghost"
													size="icon"
													class="h-7 w-7 text-destructive"
													onclick={() => handleDeletePlanSubject(s)}
												>
													<Trash2 class="w-4 h-4" />
												</Button>
											</div>
										{/each}
									</div>
									<p class="text-xs font-medium text-right mt-1">
										รวม: {tb.credits.toFixed(1)} นก · {tb.hours} ชม
									</p>
								{/if}
							</section>

							<!-- วิชาเพิ่มเติม -->
							<section>
								<h3 class="font-semibold flex items-center gap-1 mb-2 text-sm">
									<ListTodo class="w-4 h-4" /> วิชาเพิ่มเติม
								</h3>
								{#if additionalList.length === 0}
									<p class="text-xs text-muted-foreground italic">—</p>
								{:else}
									<div class="divide-y border rounded">
										{#each additionalList as s (s.id)}
											<div class="px-3 py-2 flex items-center gap-2">
												<div class="flex-1 min-w-0">
													<div class="font-medium text-sm">
														{s.subject_code}
														{s.subject_name_th ?? ''}
														{#if s.grade_level_name}
															<span class="text-xs text-muted-foreground"
																>· {s.grade_level_name}</span
															>
														{/if}
													</div>
													<div class="text-xs text-muted-foreground">
														{(s.subject_credit ?? 0).toFixed(1)} นก · {s.subject_hours ?? 0} ชม
													</div>
												</div>
												<Button
													variant="ghost"
													size="icon"
													class="h-7 w-7 text-destructive"
													onclick={() => handleDeletePlanSubject(s)}
												>
													<Trash2 class="w-4 h-4" />
												</Button>
											</div>
										{/each}
									</div>
									<p class="text-xs font-medium text-right mt-1">
										รวม: {ta.credits.toFixed(1)} นก · {ta.hours} ชม
									</p>
								{/if}
							</section>

							<!-- กิจกรรมพัฒนาผู้เรียน -->
							<section>
								<h3 class="font-semibold flex items-center gap-1 mb-2 text-sm">
									<GraduationCap class="w-4 h-4" /> กิจกรรมพัฒนาผู้เรียน
								</h3>
								{#if actList.length === 0}
									<p class="text-xs text-muted-foreground italic">—</p>
								{:else}
									<div class="divide-y border rounded">
										{#each actList as a (a.id)}
											<div class="px-3 py-2 flex items-center gap-2">
												<div class="flex-1 min-w-0">
													<div class="flex items-center gap-2 flex-wrap">
														<Badge variant="secondary" class="text-[10px]">
															{PA_TYPE_LABELS[a.catalog_activity_type ?? 'other']}
														</Badge>
														<span class="font-medium text-sm">{a.catalog_name}</span>
														{#if a.is_required}
															<Badge variant="outline" class="text-[10px]">บังคับ</Badge>
														{/if}
													</div>
													<div class="text-xs text-muted-foreground mt-1">
														{a.catalog_periods_per_week ?? 1} คาบ ·
														{a.catalog_scheduling_mode === 'independent'
															? 'แต่ละห้องจัดเอง'
															: 'จัดพร้อมกัน'}
													</div>
												</div>
												<Button
													variant="ghost"
													size="icon"
													class="h-7 w-7"
													onclick={() => openEditPlanActivity(a)}
												>
													<Pencil class="w-4 h-4" />
												</Button>
												<Button
													variant="ghost"
													size="icon"
													class="h-7 w-7 text-destructive"
													onclick={() => handleDeletePlanActivity(a)}
												>
													<Trash2 class="w-4 h-4" />
												</Button>
											</div>
										{/each}
									</div>
									<p class="text-xs font-medium text-right mt-1">
										รวม: {tact.periods} คาบ/สัปดาห์
									</p>
								{/if}
							</section>

							<!-- Grand total per term -->
							<div class="border-t pt-2 text-sm font-semibold text-right">
								รวมทั้งหมด: {(tb.credits + ta.credits).toFixed(1)} นก · {tb.hours + ta.hours} ชม
								· {tact.periods} คาบกิจกรรม
							</div>
						</div>
					{/each}
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
				<Label>
					ระดับชั้น
					<span class="text-xs text-muted-foreground">(เลือกหลายชั้นได้)</span>
				</Label>
				<div class="flex flex-wrap gap-2 p-2 border rounded min-h-[48px]">
					{#each gradeLevels as gl}
						{@const checked = (planForm.grade_level_ids ?? []).includes(gl.id)}
						<label
							class="flex items-center gap-1 text-xs border rounded px-2 py-1 cursor-pointer hover:bg-muted {checked
								? 'bg-muted font-medium'
								: ''}"
						>
							<input
								type="checkbox"
								{checked}
								onchange={(e) => {
									const ids = new Set(planForm.grade_level_ids ?? []);
									if ((e.target as HTMLInputElement).checked) ids.add(gl.id);
									else ids.delete(gl.id);
									planForm.grade_level_ids = [...ids];
								}}
							/>
							{gl.short_name ?? gl.code ?? gl.name}
						</label>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">ถ้าไม่เลือก = ทุกระดับ</p>
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

<!-- Plan Activity Dialog -->
<Dialog bind:open={showPlanActivityDialog}>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>{editingPlanActivity ? 'แก้ไขกิจกรรม' : 'เพิ่มกิจกรรม (แม่แบบ)'}</DialogTitle>
			<DialogDescription>เลือกกิจกรรมจากคลังมาใช้ในหลักสูตรนี้</DialogDescription>
		</DialogHeader>

		<div class="space-y-3 py-2">
			<div class="space-y-1">
				<Label>เลือกกิจกรรมจากคลัง *</Label>
				<Select.Root type="single" bind:value={paCatalogId}>
					<Select.Trigger class="w-full">
						{activityCatalog.find((c) => c.id === paCatalogId)?.name ?? 'เลือกกิจกรรม'}
					</Select.Trigger>
					<Select.Content class="max-h-[280px] overflow-y-auto">
						{#each activityCatalog as c}
							<Select.Item value={c.id}>
								{c.name} · {c.activity_type} · {c.periods_per_week} คาบ
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<p class="text-xs text-muted-foreground">
					ถ้าไม่มีกิจกรรมที่ต้องการ ไปเพิ่มที่หน้า
					<a href="/staff/academic/subjects" class="underline">คลังรายวิชา tab "กิจกรรม"</a>
				</p>
			</div>

			<div class="flex items-center gap-2">
				<Checkbox bind:checked={paIsRequired} id="pa-required" />
				<Label for="pa-required" class="cursor-pointer">บังคับ</Label>
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showPlanActivityDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSavePlanActivity}>บันทึก</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Add to Plan Dialog (multi-select subjects + activities) -->
<Dialog bind:open={showAddDialog}>
	<DialogContent class="max-w-3xl max-h-[85vh] overflow-y-auto">
		<DialogHeader>
			<DialogTitle>เพิ่มเข้าหลักสูตร</DialogTitle>
			<DialogDescription>
				เลือกวิชาและ/หรือกิจกรรมที่จะเพิ่มเข้าหลักสูตรนี้
			</DialogDescription>
		</DialogHeader>

		<div class="space-y-4 py-2">
			<!-- Common fields -->
			<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
				<div class="space-y-1">
					<Label>ระดับชั้น <span class="text-destructive">*</span></Label>
					<div class="flex flex-wrap gap-2">
						{#each unifiedGradeLevels as gl}
							<label
								class="flex items-center gap-1 text-xs border rounded px-2 py-1 cursor-pointer hover:bg-muted"
							>
								<input
									type="checkbox"
									checked={addGradeLevelIds.includes(gl.id)}
									onchange={(e) => {
										if ((e.target as HTMLInputElement).checked) {
											addGradeLevelIds = [...addGradeLevelIds, gl.id];
										} else {
											addGradeLevelIds = addGradeLevelIds.filter((id) => id !== gl.id);
										}
									}}
								/>
								{gl.short_name ?? gl.code}
							</label>
						{/each}
					</div>
				</div>

				<div class="space-y-1">
					<Label>ภาคเรียน</Label>
					<Select.Root type="single" bind:value={addTerm}>
						<Select.Trigger class="w-full">
							{addTerm === '1' ? 'เทอม 1' : addTerm === '2' ? 'เทอม 2' : 'ฤดูร้อน'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="1">เทอม 1</Select.Item>
							<Select.Item value="2">เทอม 2</Select.Item>
							<Select.Item value="SUMMER">ฤดูร้อน</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Subjects picker -->
			<div class="border rounded-lg p-3 space-y-2">
				<div class="flex items-center justify-between">
					<h4 class="font-semibold text-sm">วิชา ({selectedSubjectIds.size} เลือก)</h4>
					<Select.Root type="single" bind:value={addGroupFilter}>
						<Select.Trigger class="w-[200px] h-7 text-xs">
							{subjectGroups.find((g) => g.id === addGroupFilter)?.name_th ?? 'ทุกกลุ่มสาระ'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">ทุกกลุ่มสาระ</Select.Item>
							{#each subjectGroups as g}
								<Select.Item value={g.id}>{g.name_th}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="max-h-[200px] overflow-y-auto divide-y rounded border">
					{#each filteredSubjectsInDialog as s (s.id)}
						<label
							class="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-muted text-sm"
						>
							<input
								type="checkbox"
								checked={selectedSubjectIds.has(s.id)}
								onchange={() => toggleSubjectPick(s.id)}
							/>
							<span class="flex-1 truncate">
								<span class="font-medium">{s.code}</span>
								<span class="text-muted-foreground ml-1">{s.name_th}</span>
							</span>
							<Badge variant="outline" class="text-[10px]">{s.credit} นก</Badge>
						</label>
					{:else}
						<p class="text-xs text-muted-foreground italic text-center py-3">
							ไม่มีวิชาในกลุ่มนี้
						</p>
					{/each}
				</div>
			</div>

			<!-- Activities picker -->
			<div class="border rounded-lg p-3 space-y-2">
				<h4 class="font-semibold text-sm">
					กิจกรรมพัฒนาผู้เรียน ({selectedCatalogIds.size} เลือก)
				</h4>
				<div class="max-h-[180px] overflow-y-auto divide-y rounded border">
					{#each activityCatalog as c (c.id)}
						<label
							class="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-muted text-sm"
						>
							<input
								type="checkbox"
								checked={selectedCatalogIds.has(c.id)}
								onchange={() => toggleCatalogPick(c.id)}
							/>
							<span class="flex-1 truncate">{c.name}</span>
							<Badge variant="outline" class="text-[10px]">{c.activity_type}</Badge>
							<Badge variant="outline" class="text-[10px]">{c.periods_per_week} คาบ</Badge>
						</label>
					{:else}
						<p class="text-xs text-muted-foreground italic text-center py-3">
							ไม่มีกิจกรรมในคลัง
						</p>
					{/each}
				</div>
			</div>

			<div class="flex items-center gap-2">
				<Checkbox bind:checked={addIsRequired} id="add-required" />
				<Label for="add-required" class="cursor-pointer">
					บังคับ (apply ให้ทั้งวิชาและกิจกรรมที่เลือก)
				</Label>
			</div>
		</div>

		<DialogFooter>
			<Button
				variant="outline"
				onclick={() => {
					showAddDialog = false;
				}}>ยกเลิก</Button
			>
			<Button onclick={handleAddDialogSave}>บันทึก</Button>
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
