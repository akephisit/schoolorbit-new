<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();

	import {
		getAcademicStructure,
		listClassrooms,
		listSubjects,
		listClassroomCourses,
		assignCourses,
		removeCourse,
		updateCourse,
		type AcademicStructureData,
		type Classroom,
		type Subject,
		type ClassroomCourse
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
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
	import { Loader2, BookOpen, Plus, Search, Trash2, Save, Calendar, Settings } from 'lucide-svelte';

	// State
	let loading = $state(true);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });

	// Filter Selections
	let selectedYearId = $state('');
	let selectedTermId = $state('');
	let selectedClassroomId = $state('');
	let selectedTermFilter = $state('');

	// Data
	let classrooms = $state<Classroom[]>([]);
	let courses = $state<ClassroomCourse[]>([]);
	let allSubjects = $state<Subject[]>([]);

	// Dialog
	let showAddDialog = $state(false);
	let selectedSubjectIds = $state<string[]>([]);
	let subjectSearchTerm = $state('');
	let submitting = $state(false);

	// Edit Dialog
	let showEditDialog = $state(false);
	let editingCourse = $state<ClassroomCourse | null>(null);
	let deletingCourse = $state<ClassroomCourse | null>(null);
	let showDeleteDialog = $state(false);
	let teachers = $state<StaffLookupItem[]>([]);
	let selectedTeacherId = $state<string>(''); // For dropdown
	let teachersLoaded = $state(false);

	// Derived
	let filteredSemesters = $derived(
		structure.semesters.filter((s) => s.academic_year_id === selectedYearId)
	);

	let currentClassroom = $derived(classrooms.find((c) => c.id === selectedClassroomId));

	let filteredSubjects = $derived(
		allSubjects.filter((s) => {
			if (!subjectSearchTerm) return true;
			const term = subjectSearchTerm.toLowerCase();
			return (
				s.code.toLowerCase().includes(term) ||
				s.name_th.toLowerCase().includes(term) ||
				(s.name_en && s.name_en.toLowerCase().includes(term))
			);
		})
	);

	// Summary Statistics
	let summaryStats = $derived.by(() => {
		let basicCredit = 0;
		let basicHours = 0;
		let additionalCredit = 0;
		let additionalHours = 0;
		let activityHours = 0;

		courses.forEach((c) => {
			const credit = c.subject_credit || 0;
			const estimatedHours = credit * 40;

			if (c.subject_type === 'BASIC') {
				basicCredit += credit;
				basicHours += estimatedHours;
			} else if (c.subject_type === 'ADDITIONAL') {
				additionalCredit += credit;
				additionalHours += estimatedHours;
			} else if (c.subject_type === 'ACTIVITY') {
				// Activity hours might need better logic if credit is 0.
				// Assuming activities have 20-40 hours regardless of credit if we had hours fields.
				// For now, keep using estimate or if 0 credit, maybe count as 1 unit of activity time?
				// But sticking to credit-based for consistency with existing logic unless field available.
				// If credit is 0, let's treat as 20 hours (0.5 equivalent) as fallback for visible stats?
				// Or just keep 0 if no credit.
				activityHours += estimatedHours;
			}
		});

		return {
			basic: { credit: basicCredit, hours: basicHours },
			additional: { credit: additionalCredit, hours: additionalHours },
			activity: { hours: activityHours },
			total: {
				credit: basicCredit + additionalCredit,
				hours: basicHours + additionalHours + activityHours
			}
		};
	});

	// Sorted courses: Basic -> Additional -> Activity -> Others
	let sortedCourses = $derived(
		[...courses].sort((a, b) => {
			const typeOrder: Record<string, number> = { BASIC: 1, ADDITIONAL: 2, ACTIVITY: 3 };
			const orderA = typeOrder[a.subject_type || ''] || 4;
			const orderB = typeOrder[b.subject_type || ''] || 4;

			if (orderA !== orderB) return orderA - orderB;
			return (a.subject_code || '').localeCompare(b.subject_code || '');
		})
	);

	// Effects / Loaders
	async function initData() {
		try {
			loading = true;
			const res = await getAcademicStructure();
			structure = res.data;

			// Default Year
			const activeYear = structure.years.find((y) => y.is_active) || structure.years[0];
			if (activeYear) {
				selectedYearId = activeYear.id;
				await fetchClassrooms();
				await fetchClassrooms();
			}
			loadTeachers(); // Preload teachers
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

	async function loadModalSubjects() {
		try {
			const res = await listSubjects({
				academic_year_id: selectedYearId,
				term: selectedTermFilter || undefined
			});
			allSubjects = res.data;
		} catch (e) {
			toast.error('โหลดรายวิชาไม่สำเร็จ');
		}
	}

	async function openAddDialog() {
		// Auto-set filter based on current term
		const activeTerm = filteredSemesters.find((s) => s.id === selectedTermId);
		if (activeTerm) {
			selectedTermFilter = activeTerm.term;
		} else {
			selectedTermFilter = '';
		}

		await loadModalSubjects();

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

	function openDeleteDialog(course: ClassroomCourse) {
		deletingCourse = course;
		showDeleteDialog = true;
	}

	async function handleRemove() {
		if (!deletingCourse) return;

		submitting = true;
		try {
			await removeCourse(deletingCourse.id);
			toast.success('ลบวิชาสำเร็จ');
			showDeleteDialog = false;
			await fetchCourses();
		} catch (e) {
			console.error(e);
			toast.error('ลบไม่สำเร็จ');
		} finally {
			submitting = false;
		}
	}

	async function loadTeachers() {
		if (teachersLoaded) return;
		try {
			const res = await lookupStaff({ activeOnly: true, limit: 1000 });
			teachers = res;
			teachersLoaded = true;
		} catch (e) {
			console.error('Failed to load teachers', e);
		}
	}

	async function openEditDialog(course: ClassroomCourse) {
		editingCourse = course;
		selectedTeacherId = course.primary_instructor_id || 'unassigned';
		if (!teachersLoaded) await loadTeachers();
		showEditDialog = true;
	}

	async function handleUpdateCourse() {
		if (!editingCourse) return;
		submitting = true;
		try {
			const teacherId = selectedTeacherId === 'unassigned' ? null : selectedTeacherId;
			await updateCourse(editingCourse.id, {
				primary_instructor_id: teacherId
			});
			toast.success('บันทึกข้อมูลสำเร็จ');
			showEditDialog = false;
			await fetchCourses();
		} catch (e) {
			console.error(e);
			toast.error('อัปเดตข้อมูลไม่สำเร็จ');
		} finally {
			submitting = false;
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

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

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
			<!-- Summary Statistic Cards -->
			<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
				<Card.Root>
					<Card.Content class="p-3 flex flex-col items-center justify-center text-center">
						<h4 class="text-sm font-medium text-muted-foreground mb-1">วิชาพื้นฐาน</h4>
						<div class="text-xl font-bold">
							{summaryStats.basic.credit.toFixed(1)}
							<span class="text-xs font-normal text-muted-foreground">นก.</span>
						</div>
						<div class="text-xs text-muted-foreground">{summaryStats.basic.hours} ชม.</div>
					</Card.Content>
				</Card.Root>
				<Card.Root>
					<Card.Content class="p-3 flex flex-col items-center justify-center text-center">
						<h4 class="text-sm font-medium text-muted-foreground mb-1">วิชาเพิ่มเติม</h4>
						<div class="text-xl font-bold">
							{summaryStats.additional.credit.toFixed(1)}
							<span class="text-xs font-normal text-muted-foreground">นก.</span>
						</div>
						<div class="text-xs text-muted-foreground">{summaryStats.additional.hours} ชม.</div>
					</Card.Content>
				</Card.Root>
				<Card.Root>
					<Card.Content class="p-3 flex flex-col items-center justify-center text-center">
						<h4 class="text-sm font-medium text-muted-foreground mb-1">กิจกรรมฯ</h4>
						<div class="text-xl font-bold">-</div>
						<div class="text-xs text-muted-foreground">{summaryStats.activity.hours} ชม.</div>
					</Card.Content>
				</Card.Root>
				<Card.Root class="bg-primary/5 border-primary/20">
					<Card.Content class="p-3 flex flex-col items-center justify-center text-center">
						<h4 class="text-sm font-medium text-primary mb-1">รวมทั้งสิ้น</h4>
						<div class="text-xl font-bold text-primary">
							{summaryStats.total.credit.toFixed(1)}
							<span class="text-xs font-normal opacity-70">นก.</span>
						</div>
						<div class="text-xs text-muted-foreground">{summaryStats.total.hours} ชม.</div>
					</Card.Content>
				</Card.Root>
			</div>

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
							<Table.Head class="w-[100px]">ประเภท</Table.Head>
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
							{#each sortedCourses as course}
								<Table.Row>
									<Table.Cell class="font-medium">{course.subject_code}</Table.Cell>
									<Table.Cell>
										<div class="font-bold">{course.subject_name_th}</div>
										{#if course.subject_name_en}
											<div class="text-xs text-muted-foreground">{course.subject_name_en}</div>
										{/if}
									</Table.Cell>
									<Table.Cell>
										{#if course.subject_type === 'BASIC'}
											<Badge variant="outline">พื้นฐาน</Badge>
										{:else if course.subject_type === 'ADDITIONAL'}
											<Badge variant="secondary">เพิ่มเติม</Badge>
										{:else if course.subject_type === 'ACTIVITY'}
											<Badge
												variant="secondary"
												class="bg-green-100 text-green-800 hover:bg-green-100 hover:text-green-800 border-green-200"
												>กิจกรรม</Badge
											>
										{:else}
											<span class="text-muted-foreground text-xs">-</span>
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
										<Button variant="ghost" size="icon" onclick={() => openEditDialog(course)}>
											<Settings class="w-4 h-4" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											class="text-destructive hover:bg-destructive/10"
											onclick={() => openDeleteDialog(course)}
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
				<div class="flex gap-2 mb-4">
					<div class="relative flex-1">
						<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
						<Input
							type="search"
							placeholder="ค้นหารหัส หรือ ชื่อวิชา..."
							class="pl-8"
							bind:value={subjectSearchTerm}
						/>
					</div>
					<div class="w-[140px]">
						<Select.Root
							type="single"
							bind:value={selectedTermFilter}
							onValueChange={loadModalSubjects}
						>
							<Select.Trigger>
								{#if selectedTermFilter === '1'}เทอม 1
								{:else if selectedTermFilter === '2'}เทอม 2
								{:else if selectedTermFilter === 'SUMMER'}ซัมเมอร์
								{:else}ทุกเทอม{/if}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ทุกเทอม</Select.Item>
								<Select.Item value="1">เทอม 1</Select.Item>
								<Select.Item value="2">เทอม 2</Select.Item>
								<Select.Item value="SUMMER">ซัมเมอร์</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
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
	<!-- Edit Dialog -->
	<Dialog.Root bind:open={showEditDialog}>
		<Dialog.Content class="sm:max-w-[500px]">
			<Dialog.Header>
				<Dialog.Title>แก้ไขรายวิชา</Dialog.Title>
			</Dialog.Header>

			<div class="grid gap-4 py-4">
				<div class="space-y-2">
					<Label>วิชา</Label>
					<div class="font-medium p-2 border rounded bg-muted/20">
						{editingCourse?.subject_code}
						{editingCourse?.subject_name_th}
					</div>
				</div>

				<div class="space-y-2">
					<Label>ครูผู้สอน (Primary Instructor)</Label>
					<Select.Root type="single" bind:value={selectedTeacherId}>
						<Select.Trigger class="w-full">
							{#if selectedTeacherId === 'unassigned'}
								<span class="text-muted-foreground">- ไม่ระบุ -</span>
							{:else}
								{teachers.find((t) => t.id === selectedTeacherId)?.name}
							{/if}
						</Select.Trigger>
						<Select.Content class="max-h-[300px]">
							<Select.Item value="unassigned">- ไม่ระบุ -</Select.Item>
							{#each teachers as teacher}
								<Select.Item value={teacher.id}>
									{teacher.name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showEditDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleUpdateCourse} disabled={submitting}>
					{#if submitting}
						<Loader2 class="w-4 h-4 mr-2 animate-spin" />
					{/if}
					บันทึกการเปลี่ยนแปลง
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
	<!-- Delete Dialog -->
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content class="sm:max-w-[425px]">
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบวิชา</Dialog.Title>
				<Dialog.Description>
					คุณต้องการลบวิชา <strong
						>{deletingCourse?.subject_code} {deletingCourse?.subject_name_th}</strong
					> ออกจากห้องเรียนนี้ใช่หรือไม่?
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleRemove} disabled={submitting}>
					{#if submitting}
						<Loader2 class="w-4 h-4 mr-2 animate-spin" />
					{/if}
					ยืนยันลบ
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
