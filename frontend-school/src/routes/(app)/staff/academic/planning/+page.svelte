<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listClassrooms,
		listSubjects,
		listClassroomCourses,
		assignCourses,
		removeCourse,
		type AcademicStructureData,
		type Classroom,
		type Subject,
		type ClassroomCourse
	} from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { 
		Loader2, 
		BookOpen, 
		Plus, 
		Search, 
		Trash2, 
		Save,
        Calendar
	} from 'lucide-svelte';

	// State
	let loading = $state(true);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	
	// Filter Selections
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let selectedClassroomId = $state('');

	// Data
	let classrooms = $state<Classroom[]>([]);
	let courses = $state<ClassroomCourse[]>([]);
	let allSubjects = $state<Subject[]>([]);

	// Dialog
	let showAddDialog = $state(false);
	let selectedSubjectIds = $state<string[]>([]);
	let subjectSearchTerm = $state('');
	let submitting = $state(false);

	// Derived
	let filteredSemesters = $derived(
		structure.semesters.filter(s => s.academic_year_id === selectedYearId)
	);
	
	let currentClassroom = $derived(
		classrooms.find(c => c.id === selectedClassroomId)
	);

	let filteredSubjects = $derived(
		allSubjects.filter(s => {
			if (!subjectSearchTerm) return true;
			const term = subjectSearchTerm.toLowerCase();
			return (
				s.code.toLowerCase().includes(term) ||
				s.name_th.toLowerCase().includes(term) ||
				(s.name_en && s.name_en.toLowerCase().includes(term))
			);
		})
	);

	// Effects / Loaders
	async function initData() {
		try {
			loading = true;
			const res = await getAcademicStructure();
			structure = res.data;

			// Default Year
			const activeYear = structure.years.find(y => y.is_active) || structure.years[0];
			if (activeYear) {
				selectedYearId = activeYear.id;
				await fetchClassrooms();
			}
		} catch (e) {
			console.error(e);
			toast.error('โหลดข้อมูลตั้งต้นไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function fetchClassrooms() {
		if (!selectedYearId) return;
		try {
			const res = await listClassrooms({ year_id: selectedYearId });
			classrooms = res.data;
			selectedClassroomId = ''; // Reset class when year changes
		} catch (e) {
			console.error(e);
			toast.error('โหลดห้องเรียนไม่สำเร็จ');
		}
	}

	async function fetchCourses() {
		if (!selectedClassroomId || !selectedTermId) return;
		try {
			loading = true;
			const res = await listClassroomCourses(selectedClassroomId, selectedTermId);
			courses = res.data;
		} catch (e) {
			console.error(e);
			toast.error('โหลดข้อมูลรายวิชาไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function openAddDialog() {
		if (allSubjects.length === 0) {
			// Load subjects if not loaded (filtered by year ideally, but simple listSubjects filters by active year usually?)
            // We should list ALL subjects for that year.
            // listSubjects accepts { academic_year_id: ... }
			try {
				const res = await listSubjects({ academic_year_id: selectedYearId });
				allSubjects = res.data;
			} catch (e) {
				toast.error('โหลดรายวิชาไม่สำเร็จ');
				return;
			}
		}
		selectedSubjectIds = [];
		subjectSearchTerm = '';
		showAddDialog = true;
	}

	async function handleAssign() {
		if (selectedSubjectIds.length === 0) {
			toast.error('กรุณาเลือกวิชาอย่างน้อย 1 วิชา');
			return;
		}
		try {
			submitting = true;
			await assignCourses({
				classroom_id: selectedClassroomId,
				academic_semester_id: selectedTermId,
				subject_ids: selectedSubjectIds
			});
			toast.success(`เพิ่มวิชาสำเร็จ ${selectedSubjectIds.length} รายการ`);
			showAddDialog = false;
			await fetchCourses();
		} catch (e) {
			console.error(e);
			toast.error('เพิ่มวิชาไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	async function handleRemove(id: string) {
		if (!confirm('ยืนยันลบวิชานี้ออกจากห้องเรียน?')) return;
		try {
			await removeCourse(id);
			toast.success('ลบวิชาสำเร็จ');
			await fetchCourses();
		} catch (e) {
			console.error(e);
			toast.error('ลบไม่สำเร็จ');
		}
	}
	
	// Watchers (Svelte 5 effects or just bind:value + handler)
	// Using handlers on Select.onValueChange is better for explicit control?
	// But shadcn Select binds value.
	
	// When Year Changes -> Fetch Classrooms, Reset Term & Class
	function onYearChange(id: string) {
		if (id !== selectedYearId) {
			selectedYearId = id;
			selectedTermId = '';
			selectedClassroomId = '';
			fetchClassrooms();
		}
	}

	// When Class/Term Changes -> Filter Courses
	// We can use $effect
	$effect(() => {
		if (selectedClassroomId && selectedTermId) {
			fetchCourses();
		} else {
			courses = [];
		}
	});

	onMount(initData);
</script>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<BookOpen class="w-8 h-8" />
			จัดแผนการเรียน (Course Planning)
		</h2>
		<p class="text-muted-foreground">กำหนดรายวิชาที่เปิดสอนสำหรับแต่ละห้องเรียนในแต่ละภาคเรียน</p>
	</div>

	<!-- Filters -->
	<Card.Root>
		<Card.Content class="pt-6">
			<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
				<div class="space-y-2">
					<Label>ปีการศึกษา</Label>
					<Select.Root type="single" value={selectedYearId} onValueChange={onYearChange}>
						<Select.Trigger>
							{structure.years.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
						</Select.Trigger>
						<Select.Content>
							{#each structure.years as year}
								<Select.Item value={year.id}>{year.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label>ภาคเรียน (Semster)</Label>
					<Select.Root type="single" bind:value={selectedTermId}>
						<Select.Trigger disabled={!selectedYearId}>
							{filteredSemesters.find((s) => s.id === selectedTermId)?.term || 'เลือกภาคเรียน'}
						</Select.Trigger>
						<Select.Content>
							{#each filteredSemesters as term}
								<Select.Item value={term.id}>เทอม {term.term} ({term.name})</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label>ห้องเรียน</Label>
					<Select.Root type="single" bind:value={selectedClassroomId}>
						<Select.Trigger disabled={!selectedYearId}>
							{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
						</Select.Trigger>
						<Select.Content class="max-h-[300px]">
							{#each classrooms as room}
								<Select.Item value={room.id}>{room.name} ({room.grade_level_name})</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Content -->
	{#if !selectedClassroomId || !selectedTermId}
		<div
			class="h-64 flex flex-col items-center justify-center border rounded-lg bg-muted/10 text-muted-foreground border-dashed"
		>
			<Calendar class="w-12 h-12 mb-4 opacity-20" />
			<p>กรุณาเลือก ปีการศึกษา, ภาคเรียน และ ห้องเรียน เพื่อจัดการข้อมูล</p>
		</div>
	{:else}
		<div class="space-y-4">
			<div class="flex justify-between items-center">
				<h3 class="text-xl font-semibold">
					รายวิชาของห้อง {currentClassroom?.name}
					<Badge variant="outline" class="ml-2"
						>เทอม {filteredSemesters.find((s) => s.id === selectedTermId)?.term}</Badge
					>
				</h3>
				<Button onclick={openAddDialog}>
					<Plus class="w-4 h-4 mr-2" />
					เพิ่มรายวิชา
				</Button>
			</div>

			<div class="bg-card border rounded-lg overflow-hidden">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[120px]">รหัสวิชา</Table.Head>
							<Table.Head>ชื่อวิชา</Table.Head>
							<Table.Head class="text-center w-[100px]">หน่วยกิต</Table.Head>
							<Table.Head>ครูผู้สอน</Table.Head>
							<Table.Head class="text-right w-[80px]"></Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loading}
							<Table.Row>
								<Table.Cell colspan={5} class="h-32 text-center text-muted-foreground">
									<Loader2 class="w-6 h-6 animate-spin mx-auto" />
								</Table.Cell>
							</Table.Row>
						{:else if courses.length === 0}
							<Table.Row>
								<Table.Cell colspan={5} class="h-32 text-center text-muted-foreground">
									ยังไม่มีรายวิชาในภาคเรียนนี้
								</Table.Cell>
							</Table.Row>
						{:else}
							{#each courses as course}
								<Table.Row>
									<Table.Cell class="font-medium">{course.subject_code}</Table.Cell>
									<Table.Cell>
										<div class="font-bold">{course.subject_name_th}</div>
										{#if course.subject_name_en}
											<div class="text-xs text-muted-foreground">{course.subject_name_en}</div>
										{/if}
									</Table.Cell>
									<Table.Cell class="text-center">{course.subject_credit}</Table.Cell>
									<Table.Cell>
										{#if course.instructor_name}
											{course.instructor_name}
										{:else}
											<span class="text-muted-foreground text-sm">- ไม่ระบุ -</span>
										{/if}
									</Table.Cell>
									<Table.Cell class="text-right">
										<Button
											variant="ghost"
											size="icon"
											class="text-destructive hover:bg-destructive/10"
											onclick={() => handleRemove(course.id)}
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
		</div>
	{/if}

	<!-- Add Dialog -->
	<Dialog.Root bind:open={showAddDialog}>
		<Dialog.Content class="sm:max-w-[600px] max-h-[80vh] flex flex-col">
			<Dialog.Header>
				<Dialog.Title>เพิ่มรายวิชาเข้าสู่แผนการเรียน</Dialog.Title>
			</Dialog.Header>

			<div class="p-1">
				<div class="relative mb-4">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						type="search"
						placeholder="ค้นหารหัส หรือ ชื่อวิชา..."
						class="pl-8"
						bind:value={subjectSearchTerm}
					/>
				</div>

				<div class="border rounded-md overflow-hidden h-[300px] md:h-[400px]">
					<div class="overflow-y-auto h-full p-2 space-y-1">
						{#each filteredSubjects as subject}
							<label
								class="flex items-center space-x-3 p-2 rounded hover:bg-muted/50 cursor-pointer border border-transparent has-[:checked]:border-primary/20 has-[:checked]:bg-primary/5 transition-colors"
							>
								<Checkbox
									checked={selectedSubjectIds.includes(subject.id)}
									onCheckedChange={(v) => {
										if (v) selectedSubjectIds = [...selectedSubjectIds, subject.id];
										else selectedSubjectIds = selectedSubjectIds.filter((id) => id !== subject.id);
									}}
								/>
								<div class="flex-1">
									<div class="flex items-center gap-2">
										<span class="font-bold text-sm">{subject.code}</span>
										<Badge variant="outline" class="text-[10px] h-5">{subject.credit} นก.</Badge>
										{#if subject.type !== 'BASIC'}
											<Badge variant="secondary" class="text-[10px] h-5">{subject.type}</Badge>
										{/if}
									</div>
									<div class="text-sm">{subject.name_th}</div>
								</div>
							</label>
						{:else}
							<div class="text-center py-8 text-muted-foreground">ไม่พบรายวิชา</div>
						{/each}
					</div>
				</div>

				<div class="flex justify-between items-center mt-2 text-sm text-muted-foreground">
					<span>เลือกแล้ว {selectedSubjectIds.length} วิชา</span>
				</div>
			</div>

			<Dialog.Footer class="mt-auto pt-2">
				<Button variant="outline" onclick={() => (showAddDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleAssign} disabled={submitting || selectedSubjectIds.length === 0}>
					{#if submitting}
						<Loader2 class="w-4 h-4 mr-2 animate-spin" />
					{/if}
					บันทึก ({selectedSubjectIds.length})
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
