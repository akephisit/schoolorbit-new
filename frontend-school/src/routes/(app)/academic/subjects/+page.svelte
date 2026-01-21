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
        bulkCopySubjects,
		type Subject, 
		type SubjectGroup,
        type LookupItem
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
    import * as Select from '$lib/components/ui/select'; // For Batch 2
	import { BookOpen, Plus, Search, Pencil, Trash2, Copy, CircleCheck } from 'lucide-svelte';

	// Data States
	let subjects: Subject[] = $state([]);
	let groups: SubjectGroup[] = $state([]);
    let gradeLevels: LookupItem[] = $state([]);
    let academicYears: LookupItem[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	
	// Computed: Current Academic Year
	let currentAcademicYear = $derived(academicYears.find(y => y.is_current) || academicYears[0]);

	// Filter States
	let searchQuery = $state('');
	let selectedGroupId = $state('');
	let selectedSubjectType = $state('');
	let selectedLevelScope = $state('');
	let selectedYearFilter = $state('');
	let selectedYearObj = $derived(academicYears.find(y => y.id === selectedYearFilter));

	// Modal States
	let showDialog = $state(false);
	let showDeleteDialog = $state(false);
	let showCopyDialog = $state(false);
    let showSuccessDialog = $state(false);
    let successTitle = $state('');
    let successMessage = $state('');
	let selectedSourceYear = $state('');
	let copying = $state(false);
	let isEditing = $state(false);
	let submitting = $state(false);
    let deleting = $state(false);
	let currentSubject: Partial<Subject> = $state(getInitialSubjectState());

	function getInitialSubjectState(): Partial<Subject> {
		// Find current/active academic year from the list, or use first one
		const currentYear = academicYears.find(y => y.is_current) || academicYears[0];
		
		return {
			code: '',
			academic_year_id: currentYear?.id || '', // Default to current year UUID
			name_th: '',
			name_en: '',
			credit: 1.0,
			hours_per_semester: 40,
			type: 'BASIC',
			group_id: '',
			level_scope: 'ALL',
			description: '',
			is_active: true
		};
	}

	async function initData() {
		try {
			loading = true;
			// Load lookups first
			const [groupsRes, levelsRes, yearsRes] = await Promise.all([
				listSubjectGroups(),
				lookupGradeLevels(),
				lookupAcademicYears(false)
			]);

			groups = groupsRes.data;
			gradeLevels = levelsRes.data;
			academicYears = yearsRes.data;

			// Set default year filter to current year
			const current = academicYears.find(y => y.is_current);
			if (current) {
				selectedYearFilter = current.id;
			} else if (academicYears.length > 0) {
				selectedYearFilter = academicYears[0].id; // Fallback
			}

			// Then load subjects
			await loadSubjects();
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
				academic_year_id: selectedYearFilter || undefined
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
		isEditing = false;
		showDialog = true;
	}

	function handleOpenEdit(subject: Subject) {
		currentSubject = { ...subject }; // Clone
		isEditing = true;
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
			if (isEditing && currentSubject.id) {
				await updateSubject(currentSubject.id, currentSubject);
			} else {
				await createSubject(currentSubject);
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
        selectedGroupId = '';
        selectedSubjectType = '';
        selectedLevelScope = '';
        loadData();
    }

	function handleOpenCopy() {
		// Target is the selected year (or current active year if no filter selected, though filter usually selected)
		const targetId = selectedYearFilter || currentAcademicYear?.id;
        const targetObj = academicYears.find(y => y.id === targetId);
		
        if (!targetObj) return;

		// Find potential source years (exclude target AND newer years)
        // Assume 'year' field exists and is number. If missing, assume 0.
		const otherYears = academicYears.filter(y => y.id !== targetId && (y.year || 0) < (targetObj.year || 0));

		if (otherYears.length > 0) {
			selectedSourceYear = otherYears[0].id;
		} else {
             selectedSourceYear = '';
        }
		showCopyDialog = true;
	}

	async function handleBulkCopy() {
		const targetId = selectedYearFilter || currentAcademicYear?.id;
		if (!selectedSourceYear || !targetId) return;
		
		copying = true;
		try {
			const result = await bulkCopySubjects(selectedSourceYear, targetId);
			showCopyDialog = false;
            
            successTitle = 'คัดลอกรายวิชาสำเร็จ';
            successMessage = result.data.message;
            showSuccessDialog = true;
            
			await loadData(); // Reload subjects
		} catch (e) {
			alert(e instanceof Error ? e.message : 'เกิดข้อผิดพลาด');
		} finally {
			copying = false;
		}
	}

	onMount(() => {
		initData();
	});
</script>

<svelte:head>
	<title>จัดการรายวิชา - SchoolOrbit</title>
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
			<Button variant="outline" onclick={handleOpenCopy} class="flex items-center gap-2">
				<Copy class="w-4 h-4" />
				คัดลอกจากปีก่อน
			</Button>
			<Button onclick={handleOpenCreate} class="flex items-center gap-2">
				<Plus class="w-4 h-4" />
				เพิ่มรายวิชา
			</Button>
		</div>
	</div>

	<!-- Filters & Search -->
	<!-- Filters & Search -->
	<div
		class="bg-card border border-border rounded-lg p-4 flex flex-col md:flex-row gap-4 items-end md:items-center flex-wrap"
	>
		<!-- Search -->
		<div class="relative w-full md:w-auto md:flex-1 min-w-[200px]">
			<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
			<Input
				type="text"
				bind:value={searchQuery}
				onkeydown={(e) => e.key === 'Enter' && loadData()}
				placeholder="ค้นหารหัส หรือ ชื่อวิชา..."
				class="pl-10"
			/>
		</div>

		<!-- Year Filter -->
		<div class="w-full md:w-[200px]">
			<Select.Root type="single" bind:value={selectedYearFilter} onValueChange={() => loadData()}>
				<Select.Trigger>
					{academicYears.find((y) => y.id === selectedYearFilter)?.name || 'ทุกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="">ทุกปีการศึกษา</Select.Item>
					{#each academicYears as year}
						<Select.Item value={year.id}
							>{year.name} {year.is_current ? '(ปัจจุบัน)' : ''}</Select.Item
						>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<!-- Group Filter -->
		<div class="w-full md:w-[220px]">
			<Select.Root type="single" bind:value={selectedGroupId} onValueChange={() => loadData()}>
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
		<div class="w-full md:w-[180px]">
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
						<Table.Cell colspan={5} class="text-center h-24 text-muted-foreground">
							ไม่พบรายวิชา <Button
								variant="link"
								onclick={clearFilters}
								class="p-0 h-auto font-normal">ล้างตัวกรอง</Button
							>
						</Table.Cell>
					</Table.Row>
				{:else}
					{#each subjects as subject (subject.id)}
						<Table.Row>
							<Table.Cell class="font-medium">
								<div class="font-bold text-primary">{subject.code}</div>
								{#if subject.type !== 'BASIC'}
									<Badge variant="outline" class="mt-1 text-[10px] px-1 py-0 h-auto"
										>{subject.type}</Badge
									>
								{/if}
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
									>
										<Pencil class="w-4 h-4" />
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
			<DialogTitle>{isEditing ? 'แก้ไขรายวิชา' : 'เพิ่มรายวิชาใหม่'}</DialogTitle>
			<DialogDescription>กรอกข้อมูลรายวิชาให้ครบถ้วน รหัสวิชาห้ามซ้ำกัน</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="subject-code">รหัสวิชา <span class="text-destructive">*</span></Label>
					<Input id="subject-code" bind:value={currentSubject.code} placeholder="e.g. ท21101" />
				</div>
				<div class="space-y-2">
					<Label>สำหรับปีการศึกษา <span class="text-destructive">*</span></Label>
					<Select.Root type="single" bind:value={currentSubject.academic_year_id}>
						<Select.Trigger>
							{academicYears.find((y) => y.id === currentSubject.academic_year_id)?.name ||
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

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label class="text-muted-foreground">เริ่มใช้ตั้งแต่ (ปีหลักสูตร)</Label>
					<Select.Root type="single" bind:value={currentSubject.start_academic_year_id}>
						<Select.Trigger>
							{academicYears.find((y) => y.id === currentSubject.start_academic_year_id)?.name ||
								'(ไม่ระบุ)'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">(ไม่ระบุ)</Select.Item>
							{#each academicYears as year}
								<Select.Item value={year.id}>{year.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label>ระดับชั้น</Label>
					<Select.Root type="single" bind:value={currentSubject.level_scope}>
						<Select.Trigger>
							{#if currentSubject.level_scope === 'JUNIOR'}มัธยมต้น
							{:else if currentSubject.level_scope === 'SENIOR'}มัธยมปลาย
							{:else if currentSubject.level_scope === 'ALL'}ทุกระดับชั้น
							{:else if currentSubject.level_scope}
								{gradeLevels.find((l) => l.code === currentSubject.level_scope)?.name ||
									currentSubject.level_scope}
							{:else}เลือกระดับชั้น{/if}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								<Select.Label>ช่วงชั้น</Select.Label>
								<Select.Item value="JUNIOR">มัธยมต้น</Select.Item>
								<Select.Item value="SENIOR">มัธยมปลาย</Select.Item>
								<Select.Item value="ALL">ทุกระดับชั้น</Select.Item>
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

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label>ประเภทวิชา <span class="text-destructive">*</span></Label>
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
					<Select.Root type="single" bind:value={currentSubject.group_id}>
						<Select.Trigger class="truncate">
							{groups.find((g) => g.id === currentSubject.group_id)?.name_th || 'เลือกกลุ่มสาระ'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px]">
							{#each groups as group}
								<Select.Item value={group.id}>{group.code} - {group.name_th}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="subject-credit">หน่วยกิต (Credit)</Label>
					<Input id="subject-credit" type="number" step="0.5" bind:value={currentSubject.credit} />
				</div>
				<div class="space-y-2">
					<Label for="subject-hours">ชั่วโมง/เทอม</Label>
					<Input id="subject-hours" type="number" bind:value={currentSubject.hours_per_semester} />
				</div>
			</div>

			<div class="space-y-2">
				<Label for="subject-desc">คำอธิบายรายวิชา (สังเขป)</Label>
				<Textarea
					id="subject-desc"
					bind:value={currentSubject.description}
					placeholder="คำอธิบายรายวิชาย่อๆ..."
					class="min-h-[80px]"
				/>
			</div>
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

<!-- Copy Dialog -->
<Dialog bind:open={showCopyDialog}>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>คัดลอกรายวิชาจากปีก่อน</DialogTitle>
			<DialogDescription>
				{#if selectedYearObj}
					เลือกปีการศึกษาต้นทางที่ต้องการคัดลอกรายวิชามายังปี <strong>{selectedYearObj.name}</strong
					>
				{:else}
					กรุณาเลือกปีการศึกษาปลายทางก่อน
				{/if}
			</DialogDescription>
		</DialogHeader>

		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<Label>ปีการศึกษาต้นทาง</Label>
				{#if !selectedYearObj}
					<div class="p-3 text-sm text-destructive bg-destructive/10 rounded-md">
						โปรดเลือกปีการศึกษาที่ต้องการจัดการจากตัวกรองด้านบนก่อน
					</div>
				{:else}
					{@const filteredYears = academicYears.filter(
						(y) => y.id !== selectedYearObj.id && (y.year || 0) < (selectedYearObj.year || 0)
					)}
					{#if filteredYears.length === 0}
						<div
							class="flex h-10 w-full items-center justify-center rounded-md border border-dashed text-muted-foreground text-sm"
						>
							ไม่พบปีการศึกษาที่เก่ากว่าให้คัดลอก
						</div>
					{:else}
						<Select.Root type="single" bind:value={selectedSourceYear}>
							<Select.Trigger>
								{academicYears.find((y) => y.id === selectedSourceYear)?.name || 'เลือกปีการศึกษา'}
							</Select.Trigger>
							<Select.Content>
								{#each filteredYears as year}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					{/if}
				{/if}
			</div>

			<div class="text-sm text-muted-foreground bg-muted/50 p-3 rounded-md">
				<strong>หมายเหตุ:</strong> ระบบจะคัดลอกรายวิชาทั้งหมดจากปีที่เลือก รายวิชาที่มีรหัสซ้ำกันจะถูกข้ามไป
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showCopyDialog = false)} disabled={copying}>
				ยกเลิก
			</Button>
			<Button
				onclick={handleBulkCopy}
				disabled={copying || !selectedSourceYear || !selectedYearObj}
			>
				{copying ? 'กำลังคัดลอก...' : 'คัดลอก'}
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
