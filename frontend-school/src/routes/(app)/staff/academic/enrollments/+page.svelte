<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();

	import {
		getAcademicStructure,
		listClassrooms,
		getEnrollments,
		enrollStudents,
		removeEnrollment,
		autoAssignClassNumbers,
		type AcademicStructureData,
		type Classroom,
		type StudentEnrollment
	} from '$lib/api/academic';
	import { lookupStudents, type StudentLookupItem } from '$lib/api/lookup';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import UserPlus from 'lucide-svelte/icons/user-plus';
	import Trash2 from 'lucide-svelte/icons/trash-2';
	import Search from 'lucide-svelte/icons/search';
	import GraduationCap from 'lucide-svelte/icons/graduation-cap';
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import ArrowUpDown from 'lucide-svelte/icons/arrow-up-down';

	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);

	// Selection State
	let selectedYearId = $state('');
	let selectedClassroomId = $state('');
	let currentClassroom = $state<Classroom | undefined>(undefined);

	// Data State
	let enrollments = $state<StudentEnrollment[]>([]);
	let loadingEnrollments = $state(false);

	// Add Student Dialog State
	let showAddDialog = $state(false);
	let studentCandidates = $state<StudentLookupItem[]>([]);
	let selectedCandidateIds = $state<string[]>([]);
	let loadingCandidates = $state(false);
	let searchQuery = $state('');
	let isSubmitting = $state(false);
	let numberingMethod = $state<'append' | 'student_code' | 'name' | 'gender_name'>('append');

	// Remove Confirm Dialog State
	let showRemoveDialog = $state(false);
	let enrollmentToRemove = $state<StudentEnrollment | null>(null);
	let isRemoving = $state(false);

	// Auto Number Dialog State
	let showAutoNumberDialog = $state(false);
	let selectedSortMethod = $state<'student_code' | 'name' | 'gender_name'>('student_code');
	let isAutoNumbering = $state(false);

	async function loadInitData() {
		try {
			const res = await getAcademicStructure();
			structure = res.data;

			// Auto-select latest active year
			const activeYear = structure.years.find((y) => y.is_active) || structure.years[0];
			if (activeYear) {
				selectedYearId = activeYear.id;
				await handleYearChange();
			}
		} catch (error) {
			console.error(error);
			toast.error('ไม่สามารถโหลดข้อมูลโครงสร้างได้');
		}
	}

	async function handleYearChange() {
		if (!selectedYearId) return;
		try {
			const res = await listClassrooms({ year_id: selectedYearId });
			classrooms = res.data;
			selectedClassroomId = ''; // Reset classroom selection
			enrollments = [];
			currentClassroom = undefined;
		} catch (error) {
			console.error(error);
			toast.error('โหลดข้อมูลห้องเรียนไม่สำเร็จ');
		}
	}

	async function handleClassroomChange() {
		if (!selectedClassroomId) {
			enrollments = [];
			currentClassroom = undefined;
			return;
		}

		currentClassroom = classrooms.find((c) => c.id === selectedClassroomId);
		await fetchEnrollments();
	}

	async function fetchEnrollments() {
		try {
			loadingEnrollments = true;
			const res = await getEnrollments(selectedClassroomId);
			enrollments = res.data;
		} catch (error) {
			console.error(error);
			toast.error('โหลดรายชื่อนักเรียนไม่สำเร็จ');
		} finally {
			loadingEnrollments = false;
		}
	}

	async function openAddDialog() {
		showAddDialog = true;
		searchQuery = '';
		selectedCandidateIds = [];
		await searchCandidates(); // Load initial list
	}

	async function searchCandidates() {
		try {
			loadingCandidates = true;
			// Reuse existing listStudents API (Admin API)
			// Ideal: Filter only students not in this year? Or just search all.
			// Currently listStudents doesn't support complex "not in" filters easily without backend modification.
			// Let's just list all and maybe visually indicate? Or rely on user to search.
			const data = await lookupStudents({
				search: searchQuery,
				limit: 20
			});
			studentCandidates = data;
		} catch (error) {
			console.error(error);
			toast.error('ค้นหานักเรียนไม่สำเร็จ');
		} finally {
			loadingCandidates = false;
		}
	}

	function toggleCandidate(id: string) {
		if (selectedCandidateIds.includes(id)) {
			selectedCandidateIds = selectedCandidateIds.filter((cid) => cid !== id);
		} else {
			selectedCandidateIds = [...selectedCandidateIds, id];
		}
	}

	async function handleAddStudents() {
		if (selectedCandidateIds.length === 0) return;

		isSubmitting = true;
		try {
			await enrollStudents({
				student_ids: selectedCandidateIds,
				class_room_id: selectedClassroomId,
				numbering_method: numberingMethod
			});

			toast.success(`เพิ่มนักเรียน ${selectedCandidateIds.length} คน เรียบร้อยแล้ว`);
			showAddDialog = false;
			await fetchEnrollments();
		} catch (error) {
			console.error(error);
			toast.error('เพิ่มนักเรียนไม่สำเร็จ');
		} finally {
			isSubmitting = false;
		}
	}

	function openRemoveDialog(enrollment: StudentEnrollment) {
		enrollmentToRemove = enrollment;
		showRemoveDialog = true;
	}

	async function confirmRemoveStudent() {
		if (!enrollmentToRemove) return;

		isRemoving = true;
		try {
			await removeEnrollment(enrollmentToRemove.id);
			toast.success('ลบนักเรียนเรียบร้อยแล้ว');
			showRemoveDialog = false;
			enrollmentToRemove = null;
			await fetchEnrollments();
		} catch (error) {
			console.error(error);
			toast.error('ลบไม่สำเร็จ');
		} finally {
			isRemoving = false;
		}
	}

	function openAutoNumberDialog() {
		showAutoNumberDialog = true;
		selectedSortMethod = 'student_code';
	}

	async function handleAutoAssignNumbers() {
		if (!selectedClassroomId) return;

		isAutoNumbering = true;
		try {
			await autoAssignClassNumbers(selectedClassroomId, selectedSortMethod);
			toast.success('เรียงเลขที่เรียบร้อยแล้ว');
			showAutoNumberDialog = false;
			await fetchEnrollments();
		} catch (error) {
			console.error(error);
			toast.error('ไม่สามารถเรียงเลขที่ได้');
		} finally {
			isAutoNumbering = false;
		}
	}

	onMount(loadInitData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<GraduationCap class="w-8 h-8" />
				จัดห้องเรียน
			</h2>
			<p class="text-muted-foreground mt-1">จัดการนักเรียนเข้าห้องเรียนประจำปีการศึกษา</p>
		</div>
	</div>

	<!-- Filters -->
	<Card.Root>
		<Card.Content class="pt-6">
			<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
				<div class="space-y-2">
					<Label>ปีการศึกษา</Label>
					<Select.Root type="single" bind:value={selectedYearId} onValueChange={handleYearChange}>
						<Select.Trigger class="w-full">
							{structure.years.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
							{#if structure.years.find((y) => y.id === selectedYearId)?.is_active}
								(ปัจจุบัน)
							{/if}
						</Select.Trigger>
						<Select.Content>
							{#each structure.years as year (year.id)}
								<Select.Item value={year.id}
									>{year.name} {year.is_active ? '(ปัจจุบัน)' : ''}</Select.Item
								>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-2">
					<Label>ห้องเรียน</Label>
					<Select.Root
						type="single"
						bind:value={selectedClassroomId}
						onValueChange={handleClassroomChange}
						disabled={!selectedYearId}
					>
						<Select.Trigger class="w-full">
							{classrooms.find((r) => r.id === selectedClassroomId)
								? `${classrooms.find((r) => r.id === selectedClassroomId)?.grade_level_name} - ${classrooms.find((r) => r.id === selectedClassroomId)?.name}`
								: 'เลือกห้องเรียน'}
						</Select.Trigger>
						<Select.Content>
							{#each classrooms as room (room.id)}
								<Select.Item value={room.id}>{room.grade_level_name} - {room.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Content -->
	{#if !selectedClassroomId}
		<div
			class="flex h-64 flex-col items-center justify-center rounded-md border border-dashed text-muted-foreground"
		>
			<p>กรุณาเลือกปีการศึกษาและห้องเรียน</p>
		</div>
	{:else}
		<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
			<h3 class="text-lg font-semibold flex items-center gap-2">
				รายชื่อนักเรียน
				<Badge variant="secondary">{enrollments.length} คน</Badge>
			</h3>
			<div class="flex flex-col sm:flex-row gap-2 w-full sm:w-auto">
				<Button
					variant="outline"
					onclick={openAutoNumberDialog}
					disabled={enrollments.length === 0}
					class="w-full sm:w-auto"
				>
					<ArrowUpDown class="mr-2 h-4 w-4" />
					เรียงเลขที่อัตโนมัติ
				</Button>
				<Button onclick={openAddDialog} class="w-full sm:w-auto">
					<UserPlus class="mr-2 h-4 w-4" />
					เพิ่มนักเรียนเข้าห้อง
				</Button>
			</div>
		</div>

		{#if loadingEnrollments}
			<div class="flex h-40 items-center justify-center">
				<Loader2 class="h-8 w-8 animate-spin text-primary" />
			</div>
		{:else}
			<div class="rounded-md border bg-card overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[50px]">#</Table.Head>
							<Table.Head class="w-[80px]">เลขที่</Table.Head>
							<Table.Head>รหัสนักเรียน</Table.Head>
							<Table.Head>ชื่อ-นามสกุล</Table.Head>
							<Table.Head>สถานะ</Table.Head>
							<Table.Head class="text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each enrollments as item, i (item.id)}
							<Table.Row>
								<Table.Cell>{i + 1}</Table.Cell>
								<Table.Cell class="font-mono font-medium text-center">
									{item.class_number || '-'}
								</Table.Cell>
								<Table.Cell class="font-mono">{item.student_code || '-'}</Table.Cell>
								<Table.Cell class="font-medium">{item.student_name}</Table.Cell>
								<Table.Cell>
									<Badge variant="default" class="bg-green-500">Active</Badge>
								</Table.Cell>
								<Table.Cell class="text-right">
									<Button
										variant="ghost"
										size="icon"
										class="text-red-500 hover:text-red-700 hover:bg-red-50"
										onclick={() => openRemoveDialog(item)}
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								</Table.Cell>
							</Table.Row>
						{/each}
						{#if enrollments.length === 0}
							<Table.Row>
								<Table.Cell colspan={6} class="h-32 text-center text-muted-foreground">
									ยังไม่มีนักเรียนในห้องเรียนนี้
								</Table.Cell>
							</Table.Row>
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		{/if}
	{/if}

	<!-- Add Student Dialog -->
	<Dialog.Root bind:open={showAddDialog}>
		<Dialog.Content class="sm:max-w-[700px] h-[80vh] flex flex-col p-0 gap-0">
			<Dialog.Header class="p-6 pb-2">
				<Dialog.Title>เพิ่มนักเรียนเข้าห้อง {currentClassroom?.name}</Dialog.Title>
				<Dialog.Description>
					ค้นหาและเลือกนักเรียนที่ต้องการเพิ่มเข้าห้องเรียนนี้
				</Dialog.Description>
			</Dialog.Header>

			<div class="px-6 py-2 border-b">
				<div class="relative">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						type="search"
						placeholder="ค้นหาด้วยชื่อ หรือรหัสนักเรียน..."
						class="pl-9"
						bind:value={searchQuery}
						oninput={() => {
							// Debounce could be added here
							searchCandidates();
						}}
					/>
				</div>
			</div>

			<div class="flex-1 overflow-auto p-0">
				{#if loadingCandidates}
					<div class="flex h-full items-center justify-center">
						<Loader2 class="h-8 w-8 animate-spin text-primary" />
					</div>
				{:else}
					<Table.Root>
						<Table.Header class="sticky top-0 bg-background z-10">
							<Table.Row>
								<Table.Head class="w-[50px]"></Table.Head>
								<Table.Head>รหัส</Table.Head>
								<Table.Head>ชื่อ-นามสกุล</Table.Head>
								<Table.Head>สถานะปัจจุบัน</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each studentCandidates as student (student.id)}
								{#if !student.class_room}
									<Table.Row
										class="cursor-pointer hover:bg-muted/50"
										onclick={() => toggleCandidate(student.id)}
									>
										<Table.Cell>
											<Checkbox
												checked={selectedCandidateIds.includes(student.id)}
												onCheckedChange={() => toggleCandidate(student.id)}
											/>
										</Table.Cell>
										<Table.Cell class="font-mono text-xs">{student.student_id || '-'}</Table.Cell>
										<Table.Cell>{student.title || ''}{student.name}</Table.Cell>
										<Table.Cell>
											<span
												class="text-green-600 border border-green-200 bg-green-50 px-2 py-0.5 rounded text-xs"
											>
												พร้อมเข้าห้อง
											</span>
										</Table.Cell>
									</Table.Row>
								{/if}
							{/each}
						</Table.Body>
					</Table.Root>
				{/if}
			</div>

			<!-- Numbering Method Selection -->
			<div class="px-6 py-4 border-t bg-muted/10">
				<Label class="text-sm font-semibold mb-3 block">📌 วิธีกำหนดเลขที่</Label>
				<RadioGroup.Root bind:value={numberingMethod} class="space-y-3">
					<div class="flex items-center space-x-2">
						<RadioGroup.Item value="append" id="nm-append" />
						<Label for="nm-append" class="font-normal cursor-pointer flex-1">
							<span class="font-medium">เรียงต่อท้ายอัตโนมัติ</span>
							<span class="text-muted-foreground ml-1">(แนะนำ)</span>
						</Label>
					</div>
					<div class="flex items-center space-x-2">
						<RadioGroup.Item value="student_code" id="nm-code" />
						<Label for="nm-code" class="font-normal cursor-pointer flex-1">
							<span class="font-medium">เรียงตามรหัสนักเรียน</span>
							<span class="text-muted-foreground ml-1">(เรียงทั้งห้องใหม่)</span>
						</Label>
					</div>
					<div class="flex items-center space-x-2">
						<RadioGroup.Item value="name" id="nm-name" />
						<Label for="nm-name" class="font-normal cursor-pointer flex-1">
							<span class="font-medium">เรียงตามชื่อ</span>
							<span class="text-muted-foreground ml-1">(เรียงทั้งห้องใหม่)</span>
						</Label>
					</div>
					<div class="flex items-center space-x-2">
						<RadioGroup.Item value="gender_name" id="nm-gender" />
						<Label for="nm-gender" class="font-normal cursor-pointer flex-1">
							<span class="font-medium">เรียงตามเพศ + ชื่อ</span>
							<span class="text-muted-foreground ml-1">(เรียงทั้งห้องใหม่)</span>
						</Label>
					</div>
				</RadioGroup.Root>
			</div>

			<div class="p-4 border-t bg-muted/20 flex justify-between items-center">
				<span class="text-sm font-medium">เลือกแล้ว {selectedCandidateIds.length} คน</span>
				<div class="flex gap-2">
					<Button variant="outline" onclick={() => (showAddDialog = false)}>ยกเลิก</Button>
					<Button
						onclick={handleAddStudents}
						disabled={isSubmitting || selectedCandidateIds.length === 0}
					>
						{#if isSubmitting}
							<Loader2 class="mr-2 h-4 w-4 animate-spin" />
						{/if}
						เพิ่มเข้าห้องเรียน
					</Button>
				</div>
			</div>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Remove Student Confirmation Dialog -->
	<Dialog.Root bind:open={showRemoveDialog}>
		<Dialog.Content class="sm:max-w-[400px]">
			<Dialog.Header>
				<Dialog.Title class="flex items-center gap-2 text-red-600">
					<Trash2 class="h-5 w-5" />
					ยืนยันการลบนักเรียน
				</Dialog.Title>
				<Dialog.Description>
					นักเรียนจะถูกลบออกจากห้องเรียนนี้ แต่ข้อมูลนักเรียนในระบบจะยังอยู่
				</Dialog.Description>
			</Dialog.Header>

			{#if enrollmentToRemove}
				<div class="py-4">
					<div
						class="flex items-center gap-3 p-4 bg-red-50 border border-red-200 rounded-lg dark:bg-red-950/20 dark:border-red-900"
					>
						<div
							class="flex h-10 w-10 items-center justify-center rounded-full bg-red-500 text-white text-xs font-bold"
						>
							{enrollmentToRemove.student_code || '?'}
						</div>
						<div>
							<p class="font-semibold text-red-800 dark:text-red-200">
								{enrollmentToRemove.student_name}
							</p>
							<p class="text-sm text-red-600 dark:text-red-400">
								ห้อง {currentClassroom?.name || ''}
							</p>
						</div>
					</div>
				</div>
			{/if}

			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => {
						showRemoveDialog = false;
						enrollmentToRemove = null;
					}}
				>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmRemoveStudent} disabled={isRemoving}>
					{#if isRemoving}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					ยืนยันลบ
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Auto Number Dialog -->
	<Dialog.Root bind:open={showAutoNumberDialog}>
		<Dialog.Content class="sm:max-w-[500px]">
			<Dialog.Header>
				<Dialog.Title class="flex items-center gap-2">
					<ArrowUpDown class="h-5 w-5" />
					เรียงเลขที่อัตโนมัติ
				</Dialog.Title>
				<Dialog.Description>เลือกวิธีการเรียงลำดับเลขที่นักเรียนในห้องเรียนนี้</Dialog.Description>
			</Dialog.Header>

			<div class="space-y-4 py-4">
				<Label class="text-base font-semibold">วิธีการเรียง</Label>

				<!-- Radio Group -->
				<RadioGroup.Root bind:value={selectedSortMethod} class="space-y-3">
					<!-- Sort by Student Code -->
					<div
						class="flex items-start space-x-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors {selectedSortMethod ===
						'student_code'
							? 'bg-primary/5 border-primary'
							: ''}"
					>
						<RadioGroup.Item value="student_code" id="sm-code" class="mt-1" />
						<Label for="sm-code" class="font-normal cursor-pointer flex-1">
							<div class="font-medium">📋 เรียงตามรหัสนักเรียน</div>
							<div class="text-sm text-muted-foreground mt-1">
								เรียงตามลำดับรหัสนักเรียน (เช่น 67001, 67002, ...)
							</div>
						</Label>
					</div>

					<!-- Sort by Name -->
					<div
						class="flex items-start space-x-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors {selectedSortMethod ===
						'name'
							? 'bg-primary/5 border-primary'
							: ''}"
					>
						<RadioGroup.Item value="name" id="sm-name" class="mt-1" />
						<Label for="sm-name" class="font-normal cursor-pointer flex-1">
							<div class="font-medium">📝 เรียงตามชื่อ (ก-ฮ)</div>
							<div class="text-sm text-muted-foreground mt-1">
								เรียงตามลำดับตัวอักษรของชื่อนักเรียน
							</div>
						</Label>
					</div>

					<!-- Sort by Gender + Name -->
					<div
						class="flex items-start space-x-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors {selectedSortMethod ===
						'gender_name'
							? 'bg-primary/5 border-primary'
							: ''}"
					>
						<RadioGroup.Item value="gender_name" id="sm-gender" class="mt-1" />
						<Label for="sm-gender" class="font-normal cursor-pointer flex-1">
							<div class="font-medium">👥 เรียงตามเพศ + ชื่อ</div>
							<div class="text-sm text-muted-foreground mt-1">
								• ชาย (ด.ช., นาย) เรียงก่อน<br />
								• หญิง (ด.ญ., น.ส.) เรียงตาม<br />
								• ภายในเพศเดียวกันเรียงตามชื่อ
							</div>
						</Label>
					</div>
				</RadioGroup.Root>

				<div
					class="bg-yellow-50 dark:bg-yellow-950/20 border border-yellow-200 dark:border-yellow-900 rounded-lg p-3"
				>
					<p class="text-sm text-yellow-800 dark:text-yellow-200">
						⚠️ การเรียงเลขที่จะเขียนทับเลขที่เดิมทั้งหมด
					</p>
				</div>
			</div>

			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => {
						showAutoNumberDialog = false;
					}}
				>
					ยกเลิก
				</Button>
				<Button onclick={handleAutoAssignNumbers} disabled={isAutoNumbering}>
					{#if isAutoNumbering}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					✓ เรียงเลย
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
