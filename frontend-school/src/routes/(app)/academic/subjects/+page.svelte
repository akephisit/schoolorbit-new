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
	import { BookOpen, Plus, Search, Pencil, Trash2, Copy } from 'lucide-svelte';

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
			alert(result.data.message);
			showCopyDialog = false;
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
	<div
		class="bg-card border border-border rounded-lg p-4 grid gap-4 md:grid-cols-[1fr_auto_auto_auto_auto]"
	>
		<!-- Search -->
		<div class="relative">
			<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
			<Input
				type="text"
				bind:value={searchQuery}
				onkeydown={(e) => e.key === 'Enter' && loadData()}
				placeholder="ค้นหารหัส หรือ ชื่อวิชา..."
				class="pl-10 w-full"
			/>
		</div>

		<!-- Filters -->
		<select
			bind:value={selectedYearFilter}
			onchange={loadData}
			class="flex h-10 w-full md:w-[150px] items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
		>
			<option value="">ทุกปีการศึกษา</option>
			{#each academicYears as year}
				<option value={year.id}>{year.name} {year.is_current ? '(ปัจจุบัน)' : ''}</option>
			{/each}
		</select>

		<select
			bind:value={selectedGroupId}
			onchange={loadData}
			class="flex h-10 w-full md:w-[200px] items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
		>
			<option value="">ทุกกลุ่มสาระฯ</option>
			{#each groups as group}
				<option value={group.id}>{group.name_th}</option>
			{/each}
		</select>

		<select
			bind:value={selectedLevelScope}
			onchange={loadData}
			class="flex h-10 w-full md:w-[150px] items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
		>
			<option value="">ทุกระดับชั้น</option>
			<optgroup label="ช่วงชั้น">
				<option value="JUNIOR">มัธยมต้น (ม.1-3)</option>
				<option value="SENIOR">มัธยมปลาย (ม.4-6)</option>
				<option value="ALL">ทุกระดับ</option>
			</optgroup>
			{#if gradeLevels.length > 0}
				<optgroup label="ระดับชั้นเรียน (Specific)">
					{#each gradeLevels as level}
						<option value={level.code}>{level.name}</option>
					{/each}
				</optgroup>
			{/if}
		</select>

		<select
			bind:value={selectedSubjectType}
			onchange={loadData}
			class="flex h-10 w-full md:w-[150px] items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
		>
			<option value="">ทุกประเภท</option>
			<option value="BASIC">วิชาพื้นฐาน</option>
			<option value="ADDITIONAL">วิชาเพิ่มเติม</option>
			<option value="ACTIVITY">กิจกรรมฯ</option>
		</select>

		<Button variant="secondary" onclick={loadData}>ค้นหา</Button>
	</div>

	<!-- List Table -->
	<div class="bg-card border border-border rounded-lg overflow-hidden">
		<!-- Table Header -->
		<div
			class="bg-muted/50 px-6 py-3 border-b border-border text-sm font-medium text-muted-foreground hidden md:grid md:grid-cols-12 md:gap-4"
		>
			<div class="col-span-2">รหัสวิชา</div>
			<div class="col-span-4">ชื่อรายวิชา</div>
			<div class="col-span-2">กลุ่มสาระฯ</div>
			<div class="col-span-2 text-center">หน่วยกิต</div>
			<div class="col-span-2 text-right">จัดการ</div>
		</div>

		<!-- Table Body -->
		<div class="divide-y divide-border">
			{#if loading}
				<div class="p-8 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
			{:else if subjects.length === 0}
				<div class="p-8 text-center text-muted-foreground">
					ไม่พบรายวิชา <Button variant="link" onclick={clearFilters} class="p-0 h-auto font-normal"
						>ล้างตัวกรอง</Button
					>
				</div>
			{:else}
				{#each subjects as subject (subject.id)}
					<div
						class="px-6 py-4 hover:bg-accent/50 transition-colors grid grid-cols-1 md:grid-cols-12 gap-4 items-center"
					>
						<!-- Code -->
						<div class="col-span-2">
							<div class="font-bold text-primary">{subject.code}</div>
							<span class="md:hidden text-xs text-muted-foreground font-normal ml-2"
								>({subject.type})</span
							>
						</div>

						<!-- Name -->
						<div class="col-span-4">
							<div class="font-medium">{subject.name_th}</div>
							{#if subject.name_en}<div class="text-xs text-muted-foreground">
									{subject.name_en}
								</div>{/if}
						</div>

						<!-- Group -->
						<div class="col-span-2 text-sm">
							{#if subject.group_name_th}
								<span
									class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-secondary text-secondary-foreground"
								>
									{subject.group_name_th}
								</span>
							{:else}
								<span class="text-muted-foreground">-</span>
							{/if}
						</div>

						<!-- Credit & Hours -->
						<div class="col-span-2 text-sm text-center">
							<div class="font-bold">{subject.credit} นก.</div>
							<div class="text-xs text-muted-foreground">
								{subject.hours_per_semester || '-'} ชม./เทอม
							</div>
						</div>

						<!-- Actions -->
						<div class="col-span-2 flex justify-end gap-2">
							<Button onclick={() => handleOpenEdit(subject)} variant="ghost" size="sm">
								<Pencil class="w-4 h-4" />
							</Button>
							<Button
								onclick={() => handleOpenDelete(subject)}
								variant="ghost"
								size="sm"
								class="text-destructive hover:text-destructive"
							>
								<Trash2 class="w-4 h-4" />
							</Button>
						</div>
					</div>
				{/each}
			{/if}
		</div>
	</div>
</div>

<!-- Create/Edit Dialog -->
<Dialog bind:open={showDialog}>
	<DialogContent class="sm:max-w-[600px]">
		<DialogHeader>
			<DialogTitle>{isEditing ? 'แก้ไขรายวิชา' : 'เพิ่มรายวิชาใหม่'}</DialogTitle>
			<DialogDescription>กรอกข้อมูลรายวิชาให้ครบถ้วน รหัสวิชาห้ามซ้ำกัน</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<label for="subject-code" class="text-sm font-medium"
						>รหัสวิชา <span class="text-destructive">*</span></label
					>
					<Input id="subject-code" bind:value={currentSubject.code} placeholder="e.g. ท21101" />
				</div>
				<div class="space-y-2">
					<label for="subject-year" class="text-sm font-medium"
						>ปีการศึกษา <span class="text-destructive">*</span></label
					>
					<select
						id="subject-year"
						bind:value={currentSubject.academic_year_id}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						{#if academicYears.length > 0}
							{#each academicYears as year}
								<option value={year.id}>{year.name}</option>
							{/each}
						{:else}
							<option value="" disabled>กรุณาสร้างปีการศึกษาก่อน</option>
						{/if}
					</select>
				</div>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				<div class="space-y-2">
					<label for="start-year" class="text-sm font-medium text-muted-foreground"
						>เริ่มใช้ตั้งแต่ (ปีหลักสูตร)</label
					>
					<select
						id="start-year"
						bind:value={currentSubject.start_academic_year_id}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						<option value="">(ไม่ระบุ)</option>
						{#each academicYears as year}
							<option value={year.id}>{year.name}</option>
						{/each}
					</select>
				</div>

				<div class="space-y-2">
					<label for="subject-level" class="text-sm font-medium">ระดับชั้น</label>
					<select
						id="subject-level"
						bind:value={currentSubject.level_scope}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						<optgroup label="ช่วงชั้น">
							<option value="JUNIOR">มัธยมต้น</option>
							<option value="SENIOR">มัธยมปลาย</option>
							<option value="ALL">ทุกระดับชั้น</option>
						</optgroup>
						{#if gradeLevels.length > 0}
							<optgroup label="ระดับชั้นเรียน">
								{#each gradeLevels as level}
									<option value={level.code}>{level.name}</option>
								{/each}
							</optgroup>
						{/if}
					</select>
				</div>
			</div>

			<div class="space-y-2">
				<label for="subject-name-th" class="text-sm font-medium"
					>ชื่อวิชา (ภาษาไทย) <span class="text-destructive">*</span></label
				>
				<Input id="subject-name-th" bind:value={currentSubject.name_th} placeholder="ภาษาไทย 1" />
			</div>

			<div class="space-y-2">
				<label for="subject-name-en" class="text-sm font-medium">ชื่อวิชา (English)</label>
				<Input
					id="subject-name-en"
					bind:value={currentSubject.name_en}
					placeholder="Thai Language 1"
				/>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<label for="subject-type" class="text-sm font-medium"
						>ประเภทวิชา <span class="text-destructive">*</span></label
					>
					<select
						id="subject-type"
						bind:value={currentSubject.type}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						<option value="BASIC">พื้นฐาน (Basic)</option>
						<option value="ADDITIONAL">เพิ่มเติม (Additional)</option>
						<option value="ACTIVITY">กิจกรรม (Activity)</option>
					</select>
				</div>
				<div class="space-y-2">
					<label for="subject-group" class="text-sm font-medium"
						>กลุ่มสาระฯ <span class="text-destructive">*</span></label
					>
					<select
						id="subject-group"
						bind:value={currentSubject.group_id}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						<option value="" disabled>-- เลือกกลุ่มสาระ --</option>
						{#each groups as group}
							<option value={group.id}>{group.code} - {group.name_th}</option>
						{/each}
					</select>
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<label for="subject-credit" class="text-sm font-medium">หน่วยกิต (Credit)</label>
					<Input id="subject-credit" type="number" step="0.5" bind:value={currentSubject.credit} />
				</div>
				<div class="space-y-2">
					<label for="subject-hours" class="text-sm font-medium">ชั่วโมง/เทอม</label>
					<Input id="subject-hours" type="number" bind:value={currentSubject.hours_per_semester} />
				</div>
			</div>

			<div class="space-y-2">
				<label for="subject-desc" class="text-sm font-medium">คำอธิบายรายวิชา (สังเขป)</label>
				<textarea
					id="subject-desc"
					bind:value={currentSubject.description}
					class="flex min-h-[80px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
					placeholder="คำอธิบายรายวิชาย่อๆ..."
				></textarea>
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
				<label for="source-year" class="text-sm font-medium">ปีการศึกษาต้นทาง</label>
				{#if !selectedYearObj}
					<div class="p-3 text-sm text-destructive bg-destructive/10 rounded-md">
						โปรดเลือกปีการศึกษาที่ต้องการจัดการจากตัวกรองด้านบนก่อน
					</div>
				{:else if academicYears.filter((y) => y.id !== selectedYearObj.id && (y.year || 0) < (selectedYearObj.year || 0)).length === 0}
					<div
						class="flex h-10 w-full items-center justify-center rounded-md border border-dashed text-muted-foreground text-sm"
					>
						ไม่พบปีการศึกษาที่เก่ากว่าให้คัดลอก
					</div>
				{:else}
					<select
						id="source-year"
						bind:value={selectedSourceYear}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="" disabled selected>-- เลือกปีการศึกษา --</option>
						{#each academicYears.filter((y) => y.id !== selectedYearObj.id && (y.year || 0) < (selectedYearObj.year || 0)) as year}
							<option value={year.id}>{year.name}</option>
						{/each}
					</select>
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
