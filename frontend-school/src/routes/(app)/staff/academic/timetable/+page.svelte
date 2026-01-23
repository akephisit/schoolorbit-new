<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
    import { dndzone } from 'svelte-dnd-action';
    import { flip } from 'svelte/animate';
	import {
		type TimetableEntry,
		type AcademicPeriod,
		listTimetableEntries,
		createTimetableEntry,
		deleteTimetableEntry,
		listPeriods
	} from '$lib/api/timetable';
	import {
		lookupAcademicYears,
		listClassrooms,
		listClassroomCourses,
		type Classroom
	} from '$lib/api/academic';

	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';

	import {
		CalendarDays,
		Trash2,
		Loader2,
		Clock,
		School,
		GripVertical,
		BookOpen
	} from 'lucide-svelte';

	const DAYS = [
		{ value: 'MON', label: 'จันทร์', shortLabel: 'จ' },
		{ value: 'TUE', label: 'อังคาร', shortLabel: 'อ' },
		{ value: 'WED', label: 'พุธ', shortLabel: 'พ' },
		{ value: 'THU', label: 'พฤหัสบดี', shortLabel: 'พฤ' },
		{ value: 'FRI', label: 'ศุกร์', shortLabel: 'ศ' }
	];

	// State
	let loading = $state(true);
	let timetableEntries = $state<TimetableEntry[]>([]);
	let periods = $state<AcademicPeriod[]>([]);
	let classrooms = $state<Classroom[]>([]);
	let courses = $state<any[]>([]);
	let academicYears = $state<any[]>([]);

	let selectedYearId = $state('');
	let selectedClassroomId = $state('');

    // Dnd State
    // We need to maintain a copy of the draggable list for dnd-action
    let draggableCourses = $state<any[]>([]); 
    // We don't really drag items *out* of cells in this version, only *into* cells
    // So cells don't need full dndzone state in the same way, but dnd-action requires array
    // We'll handle drops manually via handleDndConsider/Finalize

	let submitting = $state(false);

	async function loadInitialData() {
		try {
			loading = true;
			const [yearsRes] = await Promise.all([lookupAcademicYears(false)]);

			academicYears = yearsRes.data;

			if (academicYears.length > 0) {
				const activeYear = academicYears.find((y) => y.is_current) || academicYears[0];
				selectedYearId = activeYear.id;
				await loadClassrooms();
			}
		} catch (e) {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadClassrooms() {
		if (!selectedYearId) return;
		try {
			const res = await listClassrooms({ year_id: selectedYearId });
			classrooms = res.data;
		} catch (e) {
			console.error(e);
		}
	}

	async function loadPeriods() {
		if (!selectedYearId) return;
		try {
			const res = await listPeriods({ academic_year_id: selectedYearId, active_only: true });
			periods = res.data.filter((p) => p.type === 'TEACHING');
		} catch (e) {
			console.error(e);
		}
	}

	async function loadCoursesForClassroom() {
		if (!selectedClassroomId) return;
		try {
			const res = await listClassroomCourses(selectedClassroomId);
			courses = res.data;
            updateDraggableCourses(); // Sync for dnd
		} catch (e) {
			console.error(e);
		}
	}

	async function loadTimetable() {
		if (!selectedClassroomId) {
			timetableEntries = [];
			return;
		}

		try {
			const res = await listTimetableEntries({ classroom_id: selectedClassroomId });
			timetableEntries = res.data;
            updateDraggableCourses(); // Sync updated counts
		} catch (e) {
			toast.error('โหลดตารางสอนไม่สำเร็จ');
		}
	}

	async function handleDeleteEntry(entryId: string) {
		try {
			await deleteTimetableEntry(entryId);
			toast.success('ลบออกจากตารางสำเร็จ');
			loadTimetable();
		} catch (e: any) {
			toast.error(e.message || 'ลบไม่สำเร็จ');
		}
	}

	function getEntryForSlot(day: string, periodId: string): TimetableEntry | undefined {
		return timetableEntries.find((e) => e.day_of_week === day && e.period_id === periodId);
	}

	function formatTime(time?: string): string {
		if (!time) return '';
		return time.substring(0, 5);
	}

    // ============================================
    // Dnd Action Handlers
    // ============================================
    
    // Prepare draggable items (add unique ID for dnd-action if needed, though they have IDs)
    function updateDraggableCourses() {
        const courseCounts = new Map<string, number>();
		timetableEntries.forEach((entry) => {
			const count = courseCounts.get(entry.classroom_course_id) || 0;
			courseCounts.set(entry.classroom_course_id, count + 1);
		});

		draggableCourses = courses.map((course) => ({
			...course,
            // dnd-action needs a unique 'id' property, which we have
			scheduled_count: courseCounts.get(course.id) || 0
		}));
    }

    // Source List Handlers
    function handleSourceConsider(e: CustomEvent) {
        draggableCourses = e.detail.items;
    }
    function handleSourceFinalize(e: CustomEvent) {
        // When dragging out, we don't accidentally remove it from source
        // Just reload original list to "snap back" if dropped elsewhere, 
        // or keep current if dnd-action handles the copy correctly (but dnd-action moves by default)
        
        // Actually, since we want "Copy" behavior, the standard pattern is:
        // On drop elsewhere, we refresh the source list to bring the item back.
        updateDraggableCourses();
    }

    // Drop Zone Handlers (The Grid Cells)
    // Since each cell is a separate drop zone, we need to handle drops per cell
    async function handleCellFinalize(e: CustomEvent, day: string, periodId: string) {
        const items = e.detail.items;
        
        // Only accept if an item was dropped (items.length > currentItems.length)
        // But here the "cell" usually has 0 or 1 item.
        // If we dropped something into an empty cell, items.length will be 1.
        
        if (items.length > 0) {
            const droppedItem = items[items.length - 1]; // The new item
            
            // Should verify it's not a duplicate within the cell (logic handled below)
            const existingEntry = getEntryForSlot(day, periodId);
            if (existingEntry) {
                 toast.error('ช่องนี้มีรายการอยู่แล้ว กรุณาลบรายการเดิมออกก่อน');
                 return; // Do nothing, dndzone will revert visually if we don't update state
            }

            // Create Entry
            const courseCode = droppedItem.subject_code;
            const courseId = droppedItem.id;
            
            const payload = {
                classroom_course_id: courseId,
                day_of_week: day,
                period_id: periodId
            };

            try {
                // Determine if we need to show loading state locally? 
                // Creating entry...
                const res = await createTimetableEntry(payload);
                if (res.success === false) {
                     toast.error(res.message || 'พบข้อขัดแย้งในตาราง');
                     if (res.conflicts) {
                         res.conflicts.forEach((c: any) => toast.error(c.message));
                     }
                } else {
                     await loadTimetable();
                     toast.success(`เพิ่ม ${courseCode} ลงตารางสำเร็จ`);
                }
            } catch(e: any) {
                toast.error(e.message);
            }
        }
    }

	$effect(() => {
		if (selectedYearId) {
			loadClassrooms();
			loadPeriods();
		}
	});

	$effect(() => {
		if (selectedClassroomId) {
			loadCoursesForClassroom();
			loadTimetable();
		}
	});

	onMount(loadInitialData);
</script>

<div class="h-full flex flex-col space-y-4">
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<CalendarDays class="w-8 h-8" />
			จัดตารางสอน
		</h2>
		<p class="text-muted-foreground">
			ลากวิชาจากด้านซ้าย มาวางในช่องตารางด้านขวา (ระบบจะตรวจสอบการชนอัตโนมัติ)
		</p>
	</div>

	<!-- Filters -->
	<div class="flex items-center gap-4 flex-wrap">
		<div class="w-[200px]">
			<Select.Root type="single" bind:value={selectedYearId}>
				<Select.Trigger class="w-full">
					{academicYears.find((y) => y.id === selectedYearId)?.name || 'เลือกปีการศึกษา'}
				</Select.Trigger>
				<Select.Content>
					{#each academicYears as year}
						<Select.Item value={year.id}>{year.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		<div class="w-[250px]">
			<Select.Root type="single" bind:value={selectedClassroomId}>
				<Select.Trigger class="w-full">
					<School class="w-4 h-4 mr-2" />
					{classrooms.find((c) => c.id === selectedClassroomId)?.name || 'เลือกห้องเรียน'}
				</Select.Trigger>
				<Select.Content>
					{#each classrooms as classroom}
						<Select.Item value={classroom.id}>{classroom.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</div>
	</div>

	{#if !selectedClassroomId}
		<Card.Root>
			<Card.Content class="py-12 text-center">
				<School class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
				<p class="text-muted-foreground">กรุณาเลือกห้องเรียนเพื่อดูและจัดตารางสอน</p>
			</Card.Content>
		</Card.Root>
	{:else if periods.length === 0}
		<Card.Root>
			<Card.Content class="py-12 text-center">
				<Clock class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
				<p class="text-muted-foreground">
					ยังไม่มีคาบเวลาในปีนี้ กรุณาไปที่เมนู "ตั้งค่าคาบเวลา" ก่อน
				</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<div
			class="grid grid-cols-12 gap-4 flex-1 overflow-hidden"
			style="height: calc(100vh - 250px);"
		>
			<!-- Fixed height for scrolling -->

			<!-- Left Sidebar: Draggable Courses -->
			<div class="col-span-3 flex flex-col h-full bg-background rounded-lg border overflow-hidden">
				<div class="p-4 border-b bg-muted/30">
					<h3 class="font-semibold flex items-center gap-2">
						<BookOpen class="w-4 h-4" /> รายวิชา
					</h3>
					<p class="text-xs text-muted-foreground mt-1">ลากวิชาไปวางในตาราง</p>
				</div>
				<div
					class="flex-1 overflow-y-auto p-2"
					use:dndzone={{ items: draggableCourses, flipDurationMs: 300, dropTargetStyle: {} }}
					onconsider={handleSourceConsider}
					onfinalize={handleSourceFinalize}
				>
					{#each draggableCourses as course (course.id)}
						<div
							class="mb-2 p-3 bg-white border rounded-lg shadow-sm hover:shadow-md transition-shadow cursor-grab active:cursor-grabbing w-full flex items-start gap-2 group"
							animate:flip={{ duration: 300 }}
						>
							<GripVertical class="w-4 h-4 text-muted-foreground mt-0.5" />
							<div class="min-w-0">
								<div class="font-medium text-sm text-blue-900 truncate">{course.subject_code}</div>
								<div class="text-xs text-muted-foreground truncate">{course.subject_name_th}</div>
								{#if course.instructor_name}
									<div class="text-xs text-blue-600 mt-1">ครู: {course.instructor_name}</div>
								{/if}
								{#if course.scheduled_count > 0}
									<Badge
										variant="outline"
										class="mt-1 text-[10px] px-1 h-5 bg-green-50 text-green-700 border-green-200"
									>
										จัดแล้ว {course.scheduled_count}
									</Badge>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</div>

			<!-- Right: Timetable Grid -->
			<div class="col-span-9 flex flex-col h-full overflow-hidden border rounded-lg">
				<div class="overflow-auto h-full relative">
					<table class="w-full text-sm border-collapse">
						<thead class="bg-muted/50 sticky top-0 z-20 shadow-sm">
							<tr>
								<th class="p-2 border sticky left-0 z-30 bg-background w-[100px]">วัน/คาบ</th>
								{#each periods as period}
									<th class="p-2 border min-w-[140px] text-center">
										<div class="font-bold">{period.name}</div>
										<div class="text-xs text-muted-foreground font-normal">
											{formatTime(period.start_time)}-{formatTime(period.end_time)}
										</div>
									</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#if loading}
								<tr
									><td colspan={periods.length + 1} class="p-10 text-center"
										><Loader2 class="animate-spin w-8 h-8 mx-auto" /></td
									></tr
								>
							{:else}
								{#each DAYS as day}
									<tr>
										<td class="p-2 border font-bold sticky left-0 z-10 bg-background">
											{day.label}
										</td>
										{#each periods as period}
											{@const entry = getEntryForSlot(day.value, period.id)}

											<!-- Cell Area -->
											<td
												class="border p-1 align-top h-[110px] min-w-[140px] relative transition-colors hover:bg-muted/10"
											>
												{#if entry}
													<!-- Filled Slot -->
													<div
														class="h-full w-full bg-blue-50 border border-blue-200 rounded p-2 relative group flex flex-col"
													>
														<div class="font-bold text-blue-900">{entry.subject_code}</div>
														<div class="text-xs text-blue-700 line-clamp-2 mt-1 flex-1">
															{entry.subject_name_th}
														</div>
														{#if entry.instructor_name}
															<div class="text-xs text-blue-600 mt-1 flex items-center gap-1">
																<span class="w-1.5 h-1.5 rounded-full bg-blue-400"></span>
																{entry.instructor_name}
															</div>
														{/if}

														<button
															onclick={() => handleDeleteEntry(entry.id)}
															class="absolute top-1 right-1 opacity-100 sm:opacity-0 group-hover:opacity-100 p-1 hover:bg-red-100 rounded text-red-500 transition-all"
															title="ลบ"
														>
															<Trash2 class="w-3.5 h-3.5" />
														</button>
													</div>
												{:else}
													<!-- Empty Drop Zone -->
													<!-- Note: dndzone requires a list. We create a temporary list for this cell -->
													<div
														class="h-full w-full flex items-center justify-center border-2 border-dashed border-transparent hover:border-blue-300 rounded transition-all"
														use:dndzone={{
															items: [],
															flipDurationMs: 0,
															dropTargetStyle: {
																outline: '2px solid #3b82f6',
																background: '#eff6ff'
															}
														}}
														onconsider={() => {}}
														onfinalize={(e) => handleCellFinalize(e, day.value, period.id)}
													>
														<span
															class="text-xs text-muted-foreground opacity-50 pointer-events-none"
															>วางที่นี่</span
														>
													</div>
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							{/if}
						</tbody>
					</table>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
    /* Custom Scrollbar for better look */
    ::-webkit-scrollbar {
        width: 8px;
        height: 8px;
    }
    ::-webkit-scrollbar-track {
        background: transparent;
    }
    ::-webkit-scrollbar-thumb {
        background: #e2e8f0;
        border-radius: 4px;
    }
    ::-webkit-scrollbar-thumb:hover {
        background: #cbd5e1;
    }
</style>
