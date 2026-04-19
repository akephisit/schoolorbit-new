<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listSubjects,
		listSubjectGroups,
		createSubject,
		updateSubject,
		deleteSubject,
		lookupGradeLevels,
		lookupAcademicYears,
		type Subject,
		type SubjectGroup,
		type LookupItem
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
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
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
	import {
		BookOpen,
		Plus,
		Search,
		Pencil,
		Trash2,
		Copy,
		CircleCheck,
		Check,
		ChevronsUpDown,
		ChevronDown,
		ChevronRight,
		Inbox,
		Info
	} from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';

	let { data } = $props();

	// true = ครูกลุ่มสาระ (manage.department เท่านั้น) — lock group filter
	let isDeptScope = $derived(
		$can.has('academic_curriculum.manage.department') &&
		!$can.has('academic_curriculum.read.all')
	);

	// Data States
	let subjects: Subject[] = $state([]);
	let groups: SubjectGroup[] = $state([]);
	let gradeLevels: LookupItem[] = $state([]);
	let academicYears: LookupItem[] = $state([]);
	let staffList: StaffLookupItem[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Computed: Current Academic Year
	let currentAcademicYear = $derived(academicYears.find((y) => y.is_current) || academicYears[0]);

	// Filter States
	let searchQuery = $state('');
	let selectedGroupId = $state('');
	let selectedSubjectType = $state('');
	let selectedLevelScope = $state('');
	let selectedYearFilter = $state('');
	let selectedYearObj = $derived(academicYears.find((y) => y.id === selectedYearFilter));
	let showAllVersions = $state(false);

	// Modal States
	let showDialog = $state(false);
	let showDeleteDialog = $state(false);
	let showSuccessDialog = $state(false);
	let successTitle = $state('');
	let successMessage = $state('');
	let isEditing = $state(false);
	let isNewVersion = $state(false);
	let submitting = $state(false);
	let deleting = $state(false);
	let showAdvanced = $state(false);
	let currentSubject: Partial<Subject> = $state(getInitialSubjectState());

	// Version grouping: compute map of code -> versions (all versions of that code
	// in the currently-loaded list). Used to render "version เก่า" / "ปัจจุบัน"
	// badges and to show a "N versions" hint when multiple versions are loaded.
	type VersionInfo = {
		versions: Subject[]; // sorted DESC by start year
		latestId: string;
	};
	let versionsByCode = $derived.by(() => {
		const map = new Map<string, VersionInfo>();
		// group
		for (const s of subjects) {
			const arr = map.get(s.code)?.versions ?? [];
			arr.push(s);
			map.set(s.code, { versions: arr, latestId: '' });
		}
		// sort each group DESC by academic-year numeric value, then derive latestId
		for (const [code, info] of map) {
			info.versions.sort((a, b) => {
				const ay = academicYears.find((y) => y.id === a.start_academic_year_id)?.year ?? 0;
				const by = academicYears.find((y) => y.id === b.start_academic_year_id)?.year ?? 0;
				return by - ay;
			});
			info.latestId = info.versions[0]?.id ?? '';
			map.set(code, info);
		}
		return map;
	});

	/** Compose a short effective-year-range label for an older version.
	 *  "เก่า (ปี 2566 → 2567)" means: effective from 2566 until 2567 (exclusive).
	 *  If it's the oldest loaded version with no successor, falls back to just "ปี 2566". */
	function versionRangeLabel(subject: Subject): string {
		const info = versionsByCode.get(subject.code);
		if (!info) return '';
		const idx = info.versions.findIndex((v) => v.id === subject.id);
		const startYear = academicYears.find((y) => y.id === subject.start_academic_year_id)?.year;
		// "next" in chronological order = the version immediately newer than this one.
		// versions[] is DESC by year, so the newer one is at idx-1.
		const nextYear =
			idx > 0
				? academicYears.find((y) => y.id === info.versions[idx - 1].start_academic_year_id)?.year
				: undefined;
		if (startYear == null) return '';
		return nextYear != null ? `ปี ${startYear}–${nextYear}` : `ตั้งแต่ปี ${startYear}`;
	}

	function getInitialSubjectState(): Partial<Subject> {
		// Find current/active academic year from the list, or use first one
		const currentYear = academicYears.find((y) => y.is_current) || academicYears[0];

		return {
			code: '',
			start_academic_year_id: currentYear?.id || '', // effective-from year for this version
			name_th: '',
			name_en: '',
			credit: 1.0,
			hours_per_semester: 40,
			type: 'BASIC',
			group_id: '',
			level_scope: undefined,
			grade_level_ids: [],
			description: '',
			is_active: true
		};
	}

	async function initData() {
		try {
			loading = true;
			// Load lookups first
			const [groupsRes, levelsRes, yearsRes, staffRes] = await Promise.all([
				listSubjectGroups(),
				lookupGradeLevels({ current_year: false }),
				lookupAcademicYears(false),
				lookupStaff({ activeOnly: true, limit: 1000 })
			]);

			groups = groupsRes.data;
			gradeLevels = levelsRes.data;
			academicYears = yearsRes.data;
			staffList = staffRes;

			// Set default year filter to current year
			const current = academicYears.find((y) => y.is_current);
			if (current) {
				selectedYearFilter = current.id;
			} else if (academicYears.length > 0) {
				selectedYearFilter = academicYears[0].id; // Fallback
			}

			// Then load subjects
			await loadSubjects();

			// dept-scope: lock group filter to teacher's group (inferred from returned subjects)
			if (isDeptScope && subjects.length > 0 && subjects[0].group_id) {
				selectedGroupId = subjects[0].group_id;
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาดในการโหลดข้อมูล';
			console.error(e);
		} finally {
			loading = false;
		}
	}

	async function loadSubjects() {
		try {
			loading = true;
			const subjectsRes = await listSubjects({
				search: searchQuery,
				group_id: selectedGroupId || undefined,
				subject_type: selectedSubjectType || undefined,
				level_scope: selectedLevelScope || undefined,
				active_in_year_id: selectedYearFilter || undefined,
				latest_only: !showAllVersions
			});
			subjects = subjectsRes.data;
		} catch (e) {
			console.error('Error loading subjects:', e);
			// Don't show global error here to avoid blocking UI actions
		} finally {
			loading = false;
		}
	}

	// Alias for compatibility with existing calls
	const loadData = loadSubjects;

	function handleOpenCreate() {
		currentSubject = getInitialSubjectState();
		// dept-scope: pre-fill and lock group to teacher's own กลุ่มสาระ
		if (isDeptScope && selectedGroupId) {
			currentSubject.group_id = selectedGroupId;
		}
		isEditing = false;
		isNewVersion = false;
		showDialog = true;
	}

	function handleOpenEdit(subject: Subject) {
		currentSubject = { ...subject }; // Clone
		isEditing = true;
		isNewVersion = false;
		showDialog = true;
	}

	function handleOpenNewVersion(subject: Subject) {
		// Create a new version of an existing subject (same code, different start year)
		// Find next academic year after subject's current start year
		const currentYear = academicYears.find((y) => y.id === subject.start_academic_year_id);
		const sortedYears = [...academicYears].sort((a, b) => (a.year ?? 0) - (b.year ?? 0));
		const nextYear = currentYear
			? (sortedYears.find((y) => (y.year ?? 0) > (currentYear.year ?? 0)) ?? currentYear)
			: (academicYears.find((y) => y.is_current) ?? academicYears[0]);

		// Pre-fill from current subject, but drop id + reset to next year
		currentSubject = {
			code: subject.code,
			start_academic_year_id: nextYear?.id || '',
			name_th: subject.name_th,
			name_en: subject.name_en ?? '',
			type: subject.type,
			group_id: subject.group_id ?? '',
			level_scope: subject.level_scope,
			grade_level_ids: [...(subject.grade_level_ids ?? [])],
			term: subject.term ?? '',
			default_instructor_id: subject.default_instructor_id ?? '',
			credit: subject.credit,
			hours_per_semester: subject.hours_per_semester,
			description: subject.description ?? '',
			is_active: true
		};

		isEditing = false; // CREATE mode so submit INSERTs
		isNewVersion = true;
		showDialog = true;
	}

	function handleOpenDelete(subject: Subject) {
		currentSubject = { ...subject };
		showDeleteDialog = true;
	}

	async function handleSubmit() {
		if (!currentSubject.code || !currentSubject.name_th) {
			alert('กรุณากรอกรหัสวิชาและชื่อวิชาให้ครบถ้วน');
			return;
		}

		submitting = true;
		try {
			// Sanitize payload: convert empty strings to null for UUID fields to avoid 422 errors
			// Sanitize all UUID & Optional fields
			const payload = { ...currentSubject };

			// Helper: Convert empty string to null.
			// Note: Keep 0 for numbers!
			const nullify = (val: any) => (val === '' || val === undefined ? null : val);

			payload.group_id = nullify(payload.group_id);
			payload.start_academic_year_id = nullify(payload.start_academic_year_id);
			payload.default_instructor_id = nullify(payload.default_instructor_id);
			payload.level_scope = nullify(payload.level_scope);
			payload.description = nullify(payload.description);
			payload.term = nullify(payload.term);

			if (payload.credit === ('' as any)) payload.credit = null as any;
			if (payload.hours_per_semester === ('' as any)) payload.hours_per_semester = null as any;

			console.log('Submitting Subject Payload:', payload);

			if (isEditing && payload.id) {
				await updateSubject(payload.id, payload as any);
			} else {
				await createSubject(payload as any);
			}
			showDialog = false;
			await loadData();
		} catch (e) {
			alert('บันทึกไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			submitting = false;
		}
	}

	async function handleConfirmDelete() {
		if (!currentSubject.id) return;
		deleting = true;
		try {
			await deleteSubject(currentSubject.id);
			showDeleteDialog = false;
			await loadData();
		} catch (e) {
			alert('ลบไม่สำเร็จ: ' + (e instanceof Error ? e.message : ''));
		} finally {
			deleting = false;
		}
	}

	function clearFilters() {
		searchQuery = '';
		if (!isDeptScope) selectedGroupId = '';
		selectedSubjectType = '';
		selectedLevelScope = '';
		// Reset year filter back to current academic year (the default)
		const current = academicYears.find((y) => y.is_current);
		selectedYearFilter = current?.id ?? academicYears[0]?.id ?? '';
		showAllVersions = false;
		loadData();
	}

	// "Active filter" = something is set that differs from defaults.
	// selectedYearFilter defaults to current year, so a mismatch counts as active.
	let hasActiveFilters = $derived.by(() => {
		const current = academicYears.find((y) => y.is_current);
		const defaultYear = current?.id ?? academicYears[0]?.id ?? '';
		if (searchQuery) return true;
		if (!isDeptScope && selectedGroupId) return true;
		if (selectedSubjectType) return true;
		if (selectedLevelScope) return true;
		if (selectedYearFilter && selectedYearFilter !== defaultYear) return true;
		if (showAllVersions) return true;
		return false;
	});

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
				<BookOpen class="w-8 h-8" />
				คลังรายวิชา
			</h1>
			<p class="text-muted-foreground mt-1">จัดการรายชื่อวิชาและกลุ่มสาระการเรียนรู้</p>
			{#if currentAcademicYear}
				<span class="ml-2 text-primary font-medium">
					• {currentAcademicYear.name}
				</span>
			{/if}
		</div>
		<div class="flex items-center gap-2">
				<Button onclick={handleOpenCreate} class="flex items-center gap-2">
				<Plus class="w-4 h-4" />
				เพิ่มรายวิชา
			</Button>
		</div>
	</div>

	<!-- Filters & Search -->
	<!-- Filters & Search -->
	<div
		class="bg-card border border-border rounded-lg p-4 flex flex-col md:flex-row gap-3 items-end md:items-center flex-wrap"
	>
		<!-- Search -->
		<div class="w-full md:w-[240px] space-y-1">
			<div class="relative">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					type="text"
					bind:value={searchQuery}
					onkeydown={(e) => e.key === 'Enter' && loadData()}
					placeholder="ค้นหารหัสหรือชื่อวิชา..."
					class="pl-10"
				/>
			</div>
			<p class="text-[10px] text-muted-foreground pl-1">
				ค้นได้ทั้งรหัสวิชา (เช่น ท21101) และชื่อ
			</p>
		</div>

		<!-- Year Filter -->
		<div class="w-full md:w-[200px] space-y-1">
			<Select.Root type="single" bind:value={selectedYearFilter} onValueChange={() => loadData()}>
				<Select.Trigger title="แสดงวิชาทุกรหัสที่ 'มีผลบังคับ' ในปีการศึกษาที่เลือก (รวมวิชาที่เริ่มใช้ตั้งแต่ปีก่อนหน้า)">
					{academicYears.find((y) => y.id === selectedYearFilter)?.name || 'ทุกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="">ทุกเวอร์ชัน (ไม่กรองปี)</Select.Item>
					{#each academicYears as year}
						<Select.Item value={year.id}
							>{year.name} {year.is_current ? '(ปัจจุบัน)' : ''}</Select.Item
						>
					{/each}
				</Select.Content>
			</Select.Root>
			<p class="text-[10px] text-muted-foreground pl-1">วิชาที่ใช้ในปี</p>
		</div>

		<!-- Show all versions toggle -->
		<label
			class="flex items-center gap-2 text-xs cursor-pointer"
			title="ปกติแสดงเฉพาะ version ล่าสุดของแต่ละรหัสวิชา ติ๊กเพื่อดู version เก่าทั้งหมด"
		>
			<Checkbox
				checked={showAllVersions}
				onCheckedChange={(v) => {
					showAllVersions = !!v;
					loadData();
				}}
			/>
			<span>แสดง version เก่าด้วย</span>
		</label>

		<!-- Group Filter -->
		<div class="w-full md:w-[220px]">
			<Select.Root type="single" bind:value={selectedGroupId} onValueChange={() => loadData()} disabled={isDeptScope}>
				<Select.Trigger class="truncate">
					{groups.find((g) => g.id === selectedGroupId)?.name_th || 'ทุกกลุ่มสาระฯ'}
				</Select.Trigger>
				<Select.Content class="max-h-[300px]">
					<Select.Item value="">ทุกกลุ่มสาระฯ</Select.Item>
					{#each groups as group}
						<Select.Item value={group.id}>{group.name_th}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<!-- Level Filter -->
		<div class="w-full md:w-[140px]">
			<Select.Root type="single" bind:value={selectedLevelScope} onValueChange={() => loadData()}>
				<Select.Trigger>
					{#if selectedLevelScope === 'JUNIOR'}มัธยมต้น
					{:else if selectedLevelScope === 'SENIOR'}มัธยมปลาย
					{:else if selectedLevelScope === 'ALL'}ทุกระดับ
					{:else if selectedLevelScope}
						{gradeLevels.find((l) => l.code === selectedLevelScope)?.name || selectedLevelScope}
					{:else}ทุกระดับชั้น{/if}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="">ทุกระดับชั้น</Select.Item>
					<Select.Group>
						<Select.Label>ช่วงชั้น</Select.Label>
						<Select.Item value="JUNIOR">มัธยมต้น (ม.1-3)</Select.Item>
						<Select.Item value="SENIOR">มัธยมปลาย (ม.4-6)</Select.Item>
						<Select.Item value="ALL">ทุกระดับ</Select.Item>
					</Select.Group>
					{#if gradeLevels.length > 0}
						<Select.Separator />
						<Select.Group>
							<Select.Label>ระดับชั้นเรียน</Select.Label>
							{#each gradeLevels as level}
								<Select.Item value={level.code}>{level.name}</Select.Item>
							{/each}
						</Select.Group>
					{/if}
				</Select.Content>
			</Select.Root>
		</div>

		<!-- Type Filter -->
		<div class="w-full md:w-[150px]">
			<Select.Root type="single" bind:value={selectedSubjectType} onValueChange={() => loadData()}>
				<Select.Trigger>
					{#if selectedSubjectType === 'BASIC'}วิชาพื้นฐาน
					{:else if selectedSubjectType === 'ADDITIONAL'}วิชาเพิ่มเติม
					{:else if selectedSubjectType === 'ACTIVITY'}กิจกรรมฯ
					{:else}ทุกประเภท{/if}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="">ทุกประเภท</Select.Item>
					<Select.Item value="BASIC">วิชาพื้นฐาน</Select.Item>
					<Select.Item value="ADDITIONAL">วิชาเพิ่มเติม</Select.Item>
					<Select.Item value="ACTIVITY">กิจกรรมฯ</Select.Item>
				</Select.Content>
			</Select.Root>
		</div>

		<Button variant="secondary" onclick={loadData}>ค้นหา</Button>
	</div>

	<!-- List Table -->
	<div class="bg-card border border-border rounded-lg overflow-hidden">
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="w-[120px]">รหัสวิชา</Table.Head>
					<Table.Head>ชื่อรายวิชา</Table.Head>
					<Table.Head>กลุ่มสาระฯ</Table.Head>
					<Table.Head class="text-center w-[120px]">หน่วยกิต</Table.Head>
					<Table.Head class="text-right w-[100px]">จัดการ</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#if loading}
					<Table.Row>
						<Table.Cell colspan={5} class="text-center h-24 text-muted-foreground">
							กำลังโหลดข้อมูล...
						</Table.Cell>
					</Table.Row>
				{:else if subjects.length === 0}
					<Table.Row>
						<Table.Cell colspan={5} class="h-48">
							<div class="flex flex-col items-center justify-center gap-3 py-6 text-center">
								{#if hasActiveFilters}
									<Inbox class="w-10 h-10 text-muted-foreground/60" />
									<div class="text-muted-foreground">ไม่พบวิชาที่ตรงกับตัวกรอง</div>
									<Button variant="outline" size="sm" onclick={clearFilters}>
										ล้างตัวกรอง
									</Button>
								{:else}
									<Inbox class="w-10 h-10 text-muted-foreground/60" />
									<div class="font-medium">ยังไม่มีวิชาในระบบ</div>
									<div class="text-xs text-muted-foreground">
										เริ่มต้นโดยคลิก "+ เพิ่มรายวิชา" ด้านบน
									</div>
									<Button size="sm" onclick={handleOpenCreate} class="gap-2">
										<Plus class="w-4 h-4" />
										เพิ่มรายวิชา
									</Button>
								{/if}
							</div>
						</Table.Cell>
					</Table.Row>
				{:else}
					{#each subjects as subject (subject.id)}
						{@const vInfo = versionsByCode.get(subject.code)}
						{@const totalVersions = vInfo?.versions.length ?? 1}
						{@const isLatestVersion = vInfo ? vInfo.latestId === subject.id : true}
						{@const latestYearName = vInfo
							? academicYears.find(
									(y) => y.id === vInfo.versions[0]?.start_academic_year_id
								)?.name
							: undefined}
						<Table.Row>
							<Table.Cell class="font-medium align-top">
								<div class="font-bold text-primary">{subject.code}</div>
								<div class="flex flex-wrap items-center gap-1 mt-1">
									{#if totalVersions > 1}
										{#if isLatestVersion}
											<Badge
												class="text-[10px] px-1.5 py-0 h-auto bg-emerald-100 text-emerald-800 border border-emerald-200 hover:bg-emerald-100"
												title="เวอร์ชันปัจจุบันของรหัสวิชานี้"
											>
												ปัจจุบัน
											</Badge>
										{:else}
											<Badge
												variant="outline"
												class="text-[10px] px-1.5 py-0 h-auto text-muted-foreground"
												title={latestYearName
													? `เวอร์ชันล่าสุดคือ ${latestYearName}`
													: 'มีเวอร์ชันที่ใหม่กว่านี้อยู่'}
											>
												เก่า · {versionRangeLabel(subject)}
											</Badge>
										{/if}
										<Badge
											variant="secondary"
											class="text-[10px] px-1.5 py-0 h-auto font-normal"
											title="มี {totalVersions} เวอร์ชันของรหัสวิชานี้ที่โหลดอยู่"
										>
											{totalVersions} versions
										</Badge>
									{/if}
									{#if subject.type !== 'BASIC'}
										<Badge variant="outline" class="text-[10px] px-1 py-0 h-auto">
											{subject.type}
										</Badge>
									{/if}
								</div>
							</Table.Cell>
							<Table.Cell>
								<div class="font-medium">{subject.name_th}</div>
								{#if subject.name_en}
									<div class="text-xs text-muted-foreground">{subject.name_en}</div>
								{/if}
							</Table.Cell>
							<Table.Cell>
								{#if subject.group_name_th}
									<Badge variant="secondary" class="font-normal whitespace-nowrap">
										{subject.group_name_th}
									</Badge>
								{:else}
									<span class="text-muted-foreground">-</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-center">
								<div class="font-bold">{subject.credit} นก.</div>
								<div class="text-xs text-muted-foreground">
									{subject.hours_per_semester || '-'} ชม./เทอม
								</div>
							</Table.Cell>
							<Table.Cell class="text-right">
								<div class="flex justify-end gap-1">
									<Button
										onclick={() => handleOpenEdit(subject)}
										variant="ghost"
										size="icon"
										class="h-8 w-8"
										title="แก้ไขข้อมูลวิชา (กระทบทุกแผน)"
									>
										<Pencil class="w-4 h-4" />
									</Button>
									<Button
										onclick={() => handleOpenNewVersion(subject)}
										variant="ghost"
										size="icon"
										class="h-8 w-8"
										title="สร้าง version ใหม่สำหรับปีการศึกษาอื่น"
									>
										<Copy class="w-4 h-4" />
									</Button>
									<Button
										onclick={() => handleOpenDelete(subject)}
										variant="ghost"
										size="icon"
										class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
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
</div>

<!-- Create/Edit Dialog -->
<Dialog bind:open={showDialog}>
	<DialogContent class="sm:max-w-[600px] max-h-[90vh] overflow-y-auto">
		<DialogHeader>
			<DialogTitle>
				{isNewVersion
					? `สร้าง version ใหม่: ${currentSubject.code ?? ''}`
					: isEditing
						? 'แก้ไขรายวิชา'
						: 'เพิ่มรายวิชาใหม่'}
			</DialogTitle>
			<DialogDescription>กรอกข้อมูลรายวิชาให้ครบถ้วน รหัสวิชาห้ามซ้ำกัน</DialogDescription>
		</DialogHeader>

		<div class="grid gap-6 py-4">
			{#if isNewVersion}
				<div
					class="rounded-lg border border-emerald-200 bg-emerald-50 p-3 text-xs text-emerald-900 space-y-1"
				>
					<div class="font-semibold">✨ สร้าง version ใหม่ของวิชา "{currentSubject.code}"</div>
					<div>• แผนเก่าที่ใช้ version เดิมจะไม่กระทบ</div>
					<div>• เลือกปีการศึกษาที่ version ใหม่นี้เริ่มมีผล</div>
				</div>
			{:else if isEditing}
				<div
					class="rounded-lg border border-amber-200 bg-amber-50 p-3 text-xs text-amber-900 space-y-1"
				>
					<div class="font-semibold">⚠ การแก้ไขนี้กระทบทุกแผนที่ใช้วิชานี้</div>
					<div>• เหมาะสำหรับแก้ typo, ปรับคำอธิบาย, ข้อมูลเล็กน้อย</div>
					<div>
						• ถ้าต้องการ<strong>เปลี่ยนข้อมูลตามปีการศึกษา</strong> (เช่น ปรับหน่วยกิต/จำนวนคาบ) → ปิด
						dialog นี้ แล้วกดปุ่ม <strong>"สร้าง version ใหม่"</strong> ที่แถวของวิชา
					</div>
				</div>
			{/if}

			<!-- Section: ข้อมูลหลัก -->
			<section class="space-y-4">
				<h3 class="text-sm font-semibold text-foreground border-b pb-1">ข้อมูลหลัก</h3>

				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label for="subject-code">รหัสวิชา <span class="text-destructive">*</span></Label>
						<Input id="subject-code" bind:value={currentSubject.code} placeholder="e.g. ท21101" />
					</div>
					<div class="space-y-2">
						<Label>ปีการศึกษาที่เริ่มใช้ <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={currentSubject.start_academic_year_id}>
							<Select.Trigger>
								{academicYears.find((y) => y.id === currentSubject.start_academic_year_id)?.name ||
									'เลือกปีการศึกษา'}
							</Select.Trigger>
							<Select.Content>
								{#if academicYears.length > 0}
									{#each academicYears as year}
										<Select.Item value={year.id}>{year.name}</Select.Item>
									{/each}
								{:else}
									<Select.Item value="" disabled>กรุณาสร้างปีการศึกษาก่อน</Select.Item>
								{/if}
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<div class="space-y-2">
					<Label for="subject-name-th"
						>ชื่อวิชา (ภาษาไทย) <span class="text-destructive">*</span></Label
					>
					<Input id="subject-name-th" bind:value={currentSubject.name_th} placeholder="ภาษาไทย 1" />
				</div>

				<div class="space-y-2">
					<Label for="subject-name-en">ชื่อวิชา (English)</Label>
					<Input
						id="subject-name-en"
						bind:value={currentSubject.name_en}
						placeholder="Thai Language 1"
					/>
				</div>
			</section>

			<!-- Section: ประเภทและระดับชั้น -->
			<section class="space-y-4">
				<h3 class="text-sm font-semibold text-foreground border-b pb-1">ประเภทและระดับชั้น</h3>

				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label
							class="flex items-center gap-1"
							title="BASIC = พื้นฐาน, ADDITIONAL = เพิ่มเติม, ACTIVITY = กิจกรรมพัฒนาผู้เรียน"
						>
							ประเภทวิชา <span class="text-destructive">*</span>
							<Info class="w-3 h-3 text-muted-foreground" />
						</Label>
						<Select.Root type="single" bind:value={currentSubject.type}>
							<Select.Trigger>
								{#if currentSubject.type === 'BASIC'}พื้นฐาน (Basic)
								{:else if currentSubject.type === 'ADDITIONAL'}เพิ่มเติม (Additional)
								{:else if currentSubject.type === 'ACTIVITY'}กิจกรรม (Activity)
								{:else}เลือกประเภท{/if}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="BASIC">พื้นฐาน (Basic)</Select.Item>
								<Select.Item value="ADDITIONAL">เพิ่มเติม (Additional)</Select.Item>
								<Select.Item value="ACTIVITY">กิจกรรม (Activity)</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label>กลุ่มสาระฯ <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={currentSubject.group_id} disabled={isDeptScope}>
							<Select.Trigger class="truncate">
								{groups.find((g) => g.id === currentSubject.group_id)?.name_th || 'เลือกกลุ่มสาระ'}
							</Select.Trigger>
							<Select.Content class="max-h-[300px]">
								{#each groups as group}
									<Select.Item value={group.id}>{group.code} - {group.name_th}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						{#if isDeptScope}
							<p class="text-[11px] text-muted-foreground">
								กลุ่มสาระที่ท่านสังกัด (ไม่สามารถเปลี่ยนได้)
							</p>
						{/if}
					</div>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label
							class="flex items-center gap-1"
							title="ระดับชั้นที่ใช้วิชานี้ เช่น ม.ต้น (ม.1-ม.3) หรือ ทุกระดับ"
						>
							ระดับชั้นที่เปิดสอน
							<Info class="w-3 h-3 text-muted-foreground" />
						</Label>
						<Popover.Root>
							<Popover.Trigger class="w-full">
								<Button
									variant="outline"
									role="combobox"
									class="w-full justify-between font-normal"
								>
									{#if currentSubject.grade_level_ids && currentSubject.grade_level_ids.length > 0}
										{currentSubject.grade_level_ids
											.map((id) => {
												const l = gradeLevels.find((l) => l.id === id);
												return l?.short_name ?? l?.code ?? id;
											})
											.join(', ')}
									{:else}
										<span class="text-muted-foreground">เลือกระดับชั้น...</span>
									{/if}
									<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
								</Button>
							</Popover.Trigger>
							<Popover.Content
								class="w-[--radix-popover-trigger-width] p-1 max-h-[260px] overflow-y-auto"
							>
								{#each gradeLevels as level}
									{@const checked = currentSubject.grade_level_ids?.includes(level.id) ?? false}
									<button
										type="button"
										class="flex items-center gap-2 w-full px-2 py-1.5 text-sm rounded hover:bg-accent text-left"
										onclick={() => {
											const ids = currentSubject.grade_level_ids ?? [];
											currentSubject.grade_level_ids = checked
												? ids.filter((id) => id !== level.id)
												: [...ids, level.id];
										}}
									>
										<Check class="h-4 w-4 {checked ? 'opacity-100' : 'opacity-0'}" />
										{level.name}
									</button>
								{/each}
							</Popover.Content>
						</Popover.Root>
						<p class="text-[10px] text-muted-foreground">ใช้สำหรับกรองรายวิชาเท่านั้น</p>
					</div>
					<div class="space-y-2">
						<Label
							class="flex items-center gap-1"
							title="เทอมที่วิชานี้จัด — ว่างไว้หมายถึงจัดได้ทุกเทอม"
						>
							ภาคเรียนที่เปิดสอน
							<Info class="w-3 h-3 text-muted-foreground" />
						</Label>
						<Select.Root type="single" bind:value={currentSubject.term}>
							<Select.Trigger>
								{#if currentSubject.term === '1'}ภาคเรียนที่ 1
								{:else if currentSubject.term === '2'}ภาคเรียนที่ 2
								{:else if currentSubject.term === 'SUMMER'}ซัมเมอร์
								{:else}ทุกภาคเรียน{/if}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ทุกภาคเรียน</Select.Item>
								<Select.Item value="1">ภาคเรียนที่ 1</Select.Item>
								<Select.Item value="2">ภาคเรียนที่ 2</Select.Item>
								<Select.Item value="SUMMER">ซัมเมอร์</Select.Item>
							</Select.Content>
						</Select.Root>
						<p class="text-[10px] text-muted-foreground">ใช้สำหรับกรองรายวิชาเท่านั้น</p>
					</div>
				</div>
			</section>

			<!-- Section: ครูและคาบเรียน -->
			<section class="space-y-4">
				<h3 class="text-sm font-semibold text-foreground border-b pb-1">ครูและคาบเรียน</h3>

				<div class="space-y-2">
					<Label
						class="flex items-center gap-1"
						title="ครูที่ระบบจะเลือกให้อัตโนมัติเมื่อสร้างคอร์ส (แก้ได้ภายหลังใน Course Planning)"
					>
						ครูผู้สอนหลัก (Default)
						<Info class="w-3 h-3 text-muted-foreground" />
					</Label>
					<Select.Root type="single" bind:value={currentSubject.default_instructor_id}>
						<Select.Trigger class="truncate">
							{(() => {
								const st = staffList.find((s) => s.id === currentSubject.default_instructor_id);
								return st ? st.name : 'เลือกครูผู้สอน';
							})()}
						</Select.Trigger>
						<Select.Content class="max-h-[300px]">
							<Select.Item value="">(ไม่ระบุ)</Select.Item>
							{#each staffList as staff}
								<Select.Item value={staff.id}>{staff.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label for="subject-credit">หน่วยกิต (Credit)</Label>
						<Input
							id="subject-credit"
							type="number"
							step="0.5"
							bind:value={currentSubject.credit}
						/>
					</div>
					<div class="space-y-2">
						<Label
							for="subject-hours"
							class="flex items-center gap-1"
							title="จำนวนคาบเรียนรวมต่อภาคเรียน (เช่น 40 = 40 คาบ/เทอม)"
						>
							คาบ/ภาค (Hours per semester)
							<Info class="w-3 h-3 text-muted-foreground" />
						</Label>
						<Input
							id="subject-hours"
							type="number"
							bind:value={currentSubject.hours_per_semester}
						/>
					</div>
				</div>
			</section>

			<!-- Section: ขั้นสูง (collapsible) -->
			<section class="space-y-2">
				<button
					type="button"
					class="flex items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
					onclick={() => (showAdvanced = !showAdvanced)}
				>
					{#if showAdvanced}
						<ChevronDown class="w-4 h-4" />
						ซ่อนขั้นสูง
					{:else}
						<ChevronRight class="w-4 h-4" />
						แสดงขั้นสูง
					{/if}
				</button>
				{#if showAdvanced}
					<div class="space-y-4 pt-2 pl-1 border-l-2 border-border pl-4">
						<div class="space-y-2">
							<Label for="subject-desc">คำอธิบายรายวิชา (สังเขป)</Label>
							<Textarea
								id="subject-desc"
								bind:value={currentSubject.description}
								placeholder="คำอธิบายรายวิชาย่อๆ..."
								class="min-h-[80px]"
							/>
						</div>

						<div class="flex items-center gap-2">
							<Checkbox
								id="subject-is-active"
								checked={currentSubject.is_active ?? true}
								onCheckedChange={(v) => (currentSubject.is_active = !!v)}
							/>
							<Label for="subject-is-active" class="cursor-pointer font-normal">
								เปิดใช้งาน (is_active)
							</Label>
							<span class="text-[10px] text-muted-foreground"
								>ปิดไว้จะไม่แสดงในตัวเลือกตอนสร้างแผน</span
							>
						</div>
					</div>
				{/if}
			</section>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSubmit} disabled={submitting}>
				{submitting ? 'กำลังบันทึก...' : 'บันทึกข้อมูล'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Delete Confirm Dialog -->
<Dialog bind:open={showDeleteDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันลบรายวิชา</DialogTitle>
			<DialogDescription>
				คุณแน่ใจหรือไม่ที่จะลบวิชา <strong>{currentSubject.code} {currentSubject.name_th}</strong>?
				การกระทำนี้ไม่สามารถย้อนกลับได้
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleConfirmDelete} disabled={deleting}>
				{deleting ? 'กำลังลบ...' : 'ลบรายวิชา'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<!-- Success Alert Dialog -->
<Dialog bind:open={showSuccessDialog}>
	<DialogContent class="sm:max-w-md">
		<DialogHeader>
			<DialogTitle class="flex items-center gap-2 text-primary">
				<CircleCheck class="w-6 h-6" />
				{successTitle}
			</DialogTitle>
			<DialogDescription>{successMessage}</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button onclick={() => (showSuccessDialog = false)}>ตกลง</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
