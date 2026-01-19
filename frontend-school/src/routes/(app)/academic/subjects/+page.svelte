<script lang="ts">
	import { onMount } from 'svelte';
	import { 
		listSubjects, 
		listSubjectGroups, 
		createSubject, 
		updateSubject, 
		deleteSubject,
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
	import { BookOpen, Plus, Search, Pencil, Trash2 } from 'lucide-svelte';

	// Data States
	let subjects: Subject[] = $state([]);
	let groups: SubjectGroup[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Filter States
	let searchQuery = $state('');
	let selectedGroupId = $state('');
	let selectedSubjectType = $state('');
	let selectedLevelScope = $state('');

	// Modal States
	let showDialog = $state(false);
	let showDeleteDialog = $state(false);
	let isEditing = $state(false);
	let submitting = $state(false);
    let deleting = $state(false);
	let currentSubject: Partial<Subject> = $state(getInitialSubjectState());

	function getInitialSubjectState(): Partial<Subject> {
		return {
			code: '',
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

	async function loadData() {
		try {
			loading = true;
			// Parallel fetch
			const [subjectsRes, groupsRes] = await Promise.all([
				listSubjects({
					search: searchQuery,
					group_id: selectedGroupId || undefined,
					subject_type: selectedSubjectType || undefined,
                    level_scope: selectedLevelScope || undefined
				}),
				listSubjectGroups()
			]);

			subjects = subjectsRes.data;
			groups = groupsRes.data;
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาดในการโหลดข้อมูล';
			console.error(e);
		} finally {
			loading = false;
		}
	}

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

	onMount(() => {
		loadData();
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
		</div>
		<Button onclick={handleOpenCreate} class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			เพิ่มรายวิชา
		</Button>
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
			<option value="JUNIOR">มัธยมต้น (ม.1-3)</option>
			<option value="SENIOR">มัธยมปลาย (ม.4-6)</option>
			<option value="ALL">ทุกระดับ</option>
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
						<div class="col-span-2 font-mono font-bold text-primary">
							{subject.code}
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
					<label for="subject-level" class="text-sm font-medium">ระดับชั้น</label>
					<select
						id="subject-level"
						bind:value={currentSubject.level_scope}
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
					>
						<option value="JUNIOR">มัธยมต้น</option>
						<option value="SENIOR">มัธยมปลาย</option>
						<option value="ALL">ทุกระดับชั้น</option>
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
