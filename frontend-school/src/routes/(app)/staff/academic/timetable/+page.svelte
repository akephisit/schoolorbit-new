<script lang="ts">
    import { onMount } from 'svelte';
    import { toast } from 'svelte-sonner';
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
    import { Label } from '$lib/components/ui/label';
   
    import * as Dialog from '$lib/components/ui/dialog';
    import * as Select from '$lib/components/ui/select';
    
    import {
        CalendarDays,
        Plus,
        Trash2,
        Loader2,
        Clock,
        School
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
    
    // Dialogs
    let showAddDialog = $state(false);
    let submitting = $state(false);
    
    // Form state
    let formDay = $state('MON');
    let formPeriodId = $state('');
    let formCourseId = $state('');

    async function loadInitialData() {
        try {
            loading = true;
            const [yearsRes] = await Promise.all([
                lookupAcademicYears(false)
            ]);
            
            academicYears = yearsRes.data;
            
            if (academicYears.length > 0) {
                const activeYear = academicYears.find(y => y.is_current) || academicYears[0];
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
            periods = res.data.filter(p => p.type === 'TEACHING'); // Only teaching periods for timetable
        } catch (e) {
            console.error(e);
        }
    }

    async function loadCoursesForClassroom() {
        if (!selectedClassroomId) return;
        try {
            const res = await listClassroomCourses(selectedClassroomId);
            courses = res.data;
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
        } catch (e) {
            toast.error('โหลดตารางสอนไม่สำเร็จ');
        }
    }

    async function handleAddEntry(e: SubmitEvent) {
        e.preventDefault();
        const form = e.target as HTMLFormElement;
        const formData = new FormData(form);
        
        const payload = {
            classroom_course_id: formData.get('classroom_course_id') as string,
            day_of_week: formData.get('day_of_week') as string,
            period_id: formData.get('period_id') as string
        };

        submitting = true;
        try {
            const res = await createTimetableEntry(payload);
            
            if (res.success === false) {
                // Conflict detected
                toast.error(res.message || 'พบข้อขัด��ล้องในตาราง');
                if (res.conflicts && res.conflicts.length > 0) {
                    res.conflicts.forEach((c: any) => {
                        toast.error(c.message);
                    });
                }
            } else {
                toast.success('เพิ่มลงตารางสำเร็จ');
                showAddDialog = false;
                loadTimetable();
            }
        } catch (e: any) {
            toast.error(e.message || 'เพิ่มลงตารางไม่สำเร็จ');
        } finally {
            submitting = false;
        }
    }

    async function handleDeleteEntry(entryId: string) {
        if (!confirm('คุณต้องการลบรายการนี้ออกจากตารางใช่หรือไม่?')) return;
        
        try {
            await deleteTimetableEntry(entryId);
            toast.success('ลบออกจากตารางสำเร็จ');
            loadTimetable();
        } catch (e: any) {
            toast.error(e.message || 'ลบไม่สำเร็จ');
        }
    }

    function getEntryForSlot(day: string, periodId: string): TimetableEntry | undefined {
        return timetableEntries.find(e => e.day_of_week === day && e.period_id === periodId);
    }

    function openAddDialog() {
        formDay = 'MON';
        formPeriodId = periods[0]?.id || '';
        formCourseId = courses[0]?.id || '';
        showAddDialog = true;
    }

    function formatTime(time?: string): string {
        if (!time) return '';
        return time.substring(0, 5);
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

<div class="space-y-6">
	<div class="flex flex-col gap-2">
		<h2 class="text-3xl font-bold flex items-center gap-2">
			<CalendarDays class="w-8 h-8" />
			จัดตารางสอน
		</h2>
		<p class="text-muted-foreground">
			เลือกห้องเรียนและเพิ่มรายวิชาลงในตารางเวลา (ระบบจะตรวจสอบการชนอัตโนมัติ)
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

		<div class="ml-auto">
			<Button onclick={openAddDialog} disabled={!selectedClassroomId || courses.length === 0}>
				<Plus class="w-4 h-4 mr-2" /> เพิ่มลงตาราง
			</Button>
		</div>
	</div>

	<!-- Timetable Grid -->
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
		<Card.Root>
			<div class="overflow-x-auto">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-[100px] sticky left-0 bg-background z-10">คาบ/วัน</Table.Head>
							{#each DAYS as day}
								<Table.Head class="text-center min-w-[120px]">{day.label}</Table.Head>
							{/each}
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#if loading}
							<Table.Row>
								<Table.Cell colspan={6} class="h-24 text-center">
									<Loader2 class="animate-spin mx-auto" />
								</Table.Cell>
							</Table.Row>
						{:else}
							{#each periods as period}
								<Table.Row>
									<Table.Cell class="sticky left-0 bg-background z-10">
										<div class="font-medium">{period.name}</div>
										<div class="text-xs text-muted-foreground">
											{formatTime(period.start_time)} - {formatTime(period.end_time)}
										</div>
									</Table.Cell>
									{#each DAYS as day}
										{@const entry = getEntryForSlot(day.value, period.id)}
										<Table.Cell class="p-1">
											{#if entry}
												<div class="bg-blue-50 border border-blue-200 rounded p-2 relative group">
													<div class="font-medium text-sm text-blue-900">{entry.subject_code}</div>
													<div class="text-xs text-blue-700">{entry.subject_name_th}</div>
													{#if entry.instructor_name}
														<div class="text-xs text-muted-foreground mt-1">
															{entry.instructor_name}
														</div>
													{/if}
													<button
														onclick={() => handleDeleteEntry(entry.id)}
														class="absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity bg-red-100 hover:bg-red-200 rounded p-1"
														title="ลบออกจากตาราง"
													>
														<Trash2 class="w-3 h-3 text-red-600" />
													</button>
												</div>
											{:else}
												<div class="h-16 border border-dashed border-muted rounded"></div>
											{/if}
										</Table.Cell>
									{/each}
								</Table.Row>
							{/each}
						{/if}
					</Table.Body>
				</Table.Root>
			</div>
		</Card.Root>
	{/if}

	<!-- Add Entry Dialog -->
	<Dialog.Root bind:open={showAddDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>เพิ่มวิชาลงตาราง</Dialog.Title>
			</Dialog.Header>
			<form onsubmit={handleAddEntry} class="space-y-4 py-4">
				<input type="hidden" name="day_of_week" value={formDay} />
				<input type="hidden" name="period_id" value={formPeriodId} />
				<input type="hidden" name="classroom_course_id" value={formCourseId} />

				<div class="space-y-2">
					<Label>วิชา <span class="text-red-500">*</span></Label>
					<Select.Root type="single" bind:value={formCourseId}>
						<Select.Trigger class="w-full">
							{@const course = courses.find((c) => c.id === formCourseId)}
							{course ? `${course.subject_code} - ${course.subject_name_th}` : 'เลือกวิชา'}
						</Select.Trigger>
						<Select.Content>
							{#each courses as course}
								<Select.Item value={course.id}>
									{course.subject_code} - {course.subject_name_th}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label>วัน <span class="text-red-500">*</span></Label>
						<Select.Root type="single" bind:value={formDay}>
							<Select.Trigger class="w-full">
								{DAYS.find((d) => d.value === formDay)?.label}
							</Select.Trigger>
							<Select.Content>
								{#each DAYS as day}
									<Select.Item value={day.value}>{day.label}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="space-y-2">
						<Label>คาบ <span class="text-red-500">*</span></Label>
						<Select.Root type="single" bind:value={formPeriodId}>
							<Select.Trigger class="w-full">
								{periods.find((p) => p.id === formPeriodId)?.name || 'เลือกคาบ'}
							</Select.Trigger>
							<Select.Content>
								{#each periods as period}
									<Select.Item value={period.id}>
										{period.name} ({formatTime(period.start_time)}-{formatTime(period.end_time)})
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<Dialog.Footer>
					<Button variant="outline" type="button" onclick={() => (showAddDialog = false)}
						>ยกเลิก</Button
					>
					<Button type="submit" disabled={submitting}>เพิ่มลงตาราง</Button>
				</Dialog.Footer>
			</form>
		</Dialog.Content>
	</Dialog.Root>
</div>
