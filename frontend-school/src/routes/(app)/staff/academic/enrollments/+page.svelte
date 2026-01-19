<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listClassrooms,
		getEnrollments,
		enrollStudents,
		removeEnrollment,
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
	import { Checkbox } from '$lib/components/ui/checkbox';

	let loading = true;
	let structure: AcademicStructureData = { years: [], semesters: [], levels: [] };
	let classrooms: Classroom[] = [];
	
	// Selection State
	let selectedYearId = '';
	let selectedClassroomId = '';
	let currentClassroom: Classroom | undefined;
	
	// Data State
	let enrollments: StudentEnrollment[] = [];
	let loadingEnrollments = false;

	// Add Student Dialog State
	let showAddDialog = false;
	let studentCandidates: StudentLookupItem[] = [];
	let selectedCandidateIds: string[] = [];
	let loadingCandidates = false;
	let searchQuery = '';
	let isSubmitting = false;

	// Remove Confirm Dialog State
	let showRemoveDialog = false;
	let enrollmentToRemove: StudentEnrollment | null = null;
	let isRemoving = false;

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
		} finally {
			loading = false;
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
		
		currentClassroom = classrooms.find(c => c.id === selectedClassroomId);
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
			selectedCandidateIds = selectedCandidateIds.filter(cid => cid !== id);
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
				class_room_id: selectedClassroomId
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

	onMount(loadInitData);
</script>

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
							{#each structure.years as year}
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
							{#each classrooms as room}
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
		<div class="flex items-center justify-between">
			<h3 class="text-lg font-semibold flex items-center gap-2">
				รายชื่อนักเรียน
				<Badge variant="secondary">{enrollments.length} คน</Badge>
			</h3>
			<Button onclick={openAddDialog}>
				<UserPlus class="mr-2 h-4 w-4" />
				เพิ่มนักเรียนเข้าห้อง
			</Button>
		</div>

		{#if loadingEnrollments}
			<div class="flex h-40 items-center justify-center">
				<Loader2 class="h-8 w-8 animate-spin text-primary" />
			</div>
		{:else}
			<div class="rounded-md border bg-card">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[50px]">#</Table.Head>
							<Table.Head>รหัสนักเรียน</Table.Head>
							<Table.Head>ชื่อ-นามสกุล</Table.Head>
							<Table.Head>สถานะ</Table.Head>
							<Table.Head class="text-right">จัดการ</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each enrollments as item, i}
							<Table.Row>
								<Table.Cell>{i + 1}</Table.Cell>
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
								<Table.Cell colspan={5} class="h-32 text-center text-muted-foreground">
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
							{#each studentCandidates as student}
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
										{#if student.class_room}
											<Badge variant="outline">{student.class_room}</Badge>
										{:else}
											<span class="text-muted-foreground text-xs">- ยังไม่มีห้อง -</span>
										{/if}
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				{/if}
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
</div>
