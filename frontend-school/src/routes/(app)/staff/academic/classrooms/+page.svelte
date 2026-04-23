<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();

	import {
		getAcademicStructure,
		listClassrooms,
		createClassroom,
		updateClassroom,
		getYearLevelConfig,
		listStudyPlanVersions,
		type AcademicStructureData,
		type Classroom,
		type StudyPlanVersion
	} from '$lib/api/academic';
	import { lookupStaff, type StaffLookupItem } from '$lib/api/lookup';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
	import {
		Loader2,
		Plus,
		Users,
		School,
		Pencil,
		Trash2,
		ChevronsUpDown,
		Check
	} from 'lucide-svelte';

	type AdvisorRow = { user_id: string; role: 'primary' | 'secondary' };

	let loading = $state(true);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let activeLevelIds = $state<string[]>([]);
	let studyPlanVersions = $state<StudyPlanVersion[]>([]);

	let selectedYearId = $state('');

	let filteredStudyPlanVersions = $derived(
		studyPlanVersions.filter((v) => {
			const selectedYear = structure.years.find((y) => y.id === selectedYearId);
			if (!selectedYear) return true;
			const startYear = structure.years.find((y) => y.id === v.start_academic_year_id);
			if (!startYear) return true;
			return startYear.year === selectedYear.year;
		})
	);

	let showCreateDialog = $state(false);
	let showEditDialog = $state(false);
	let isSubmitting = $state(false);

	// Create form
	let newClassroom = $state({
		academic_year_id: '',
		grade_level_id: '',
		room_number: '',
		study_plan_version_id: '',
		capacity: 40
	});
	let newAdvisors = $state<AdvisorRow[]>([]);

	$effect(() => {
		newClassroom.academic_year_id = selectedYearId;
		newClassroom.study_plan_version_id = '';
	});

	// Edit form
	let editingClassroom = $state({
		id: '',
		room_number: '',
		study_plan_version_id: '',
		capacity: 40
	});
	let editingAdvisors = $state<AdvisorRow[]>([]);

	// Add-advisor row (shared between create + edit dialogs via flag)
	let addStaffId = $state('');
	let addRole = $state<'primary' | 'secondary'>('primary');
	let staffPickerOpen = $state(false);

	function resetAddRow(list: AdvisorRow[]) {
		addStaffId = '';
		addRole = list.some((a) => a.role === 'primary') ? 'secondary' : 'primary';
	}

	function addAdvisor(list: AdvisorRow[]): AdvisorRow[] {
		if (!addStaffId) return list;
		if (list.some((a) => a.user_id === addStaffId)) {
			toast.error('ครูคนนี้อยู่ในรายการแล้ว');
			return list;
		}
		let next = list.slice();
		// Promoting to primary → demote existing primary to secondary
		if (addRole === 'primary') {
			next = next.map((a) => (a.role === 'primary' ? { ...a, role: 'secondary' as const } : a));
		}
		next.push({ user_id: addStaffId, role: addRole });
		resetAddRow(next);
		return next;
	}

	function toggleRole(list: AdvisorRow[], userId: string): AdvisorRow[] {
		const target = list.find((a) => a.user_id === userId);
		if (!target) return list;
		const nextRole: 'primary' | 'secondary' = target.role === 'primary' ? 'secondary' : 'primary';
		return list.map((a) => {
			if (a.user_id === userId) return { ...a, role: nextRole };
			if (nextRole === 'primary' && a.role === 'primary')
				return { ...a, role: 'secondary' as const };
			return a;
		});
	}

	function removeAdvisor(list: AdvisorRow[], userId: string): AdvisorRow[] {
		return list.filter((a) => a.user_id !== userId);
	}

	function staffName(id: string): string {
		const s = staffList.find((x) => x.id === id);
		return s ? `${s.title ?? ''}${s.name}` : id;
	}

	async function loadInitData() {
		try {
			loading = true;
			const [structureRes, staffData, versionsRes] = await Promise.all([
				getAcademicStructure(),
				lookupStaff(),
				listStudyPlanVersions({ active_only: true })
			]);
			structure = structureRes.data;
			staffList = staffData;
			studyPlanVersions = versionsRes.data;

			const activeYear = structure.years.find((y) => y.is_active) || structure.years[0];
			if (activeYear) selectedYearId = activeYear.id;

			await fetchClassrooms();
		} catch (error) {
			console.error(error);
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function fetchClassrooms() {
		if (!selectedYearId) return;
		try {
			const [classroomRes, configRes] = await Promise.all([
				listClassrooms({ year_id: selectedYearId }),
				getYearLevelConfig(selectedYearId)
			]);
			classrooms = classroomRes.data;
			activeLevelIds = configRes.data;
		} catch (error) {
			console.error(error);
			toast.error('โหลดข้อมูลห้องเรียนไม่สำเร็จ');
		}
	}

	function handleOpenCreate() {
		newClassroom = {
			academic_year_id: selectedYearId,
			grade_level_id: '',
			room_number: '',
			study_plan_version_id: '',
			capacity: 40
		};
		newAdvisors = [];
		resetAddRow(newAdvisors);
		showCreateDialog = true;
	}

	async function handleCreateClassroom() {
		if (
			!newClassroom.academic_year_id ||
			!newClassroom.grade_level_id ||
			!newClassroom.room_number ||
			!newClassroom.study_plan_version_id
		) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็นให้ครบถ้วน (รวมถึงหลักสูตร)');
			return;
		}

		isSubmitting = true;
		try {
			await createClassroom({
				...newClassroom,
				advisors: newAdvisors
			});
			toast.success('สร้างห้องเรียนสำเร็จ');
			showCreateDialog = false;
			await fetchClassrooms();
		} catch (error) {
			console.error(error);
			toast.error('สร้างห้องเรียนไม่สำเร็จ (ชื่อห้องซ้ำหรือข้อมูลไม่ถูกต้อง)');
		} finally {
			isSubmitting = false;
		}
	}

	function handleOpenEdit(room: Classroom) {
		editingClassroom = {
			id: room.id,
			room_number: room.room_number || '',
			study_plan_version_id: room.study_plan_version_id || '',
			capacity: room.capacity ?? 40
		};
		editingAdvisors = (room.advisors ?? []).map((a) => ({ user_id: a.user_id, role: a.role }));
		resetAddRow(editingAdvisors);
		showEditDialog = true;
	}

	async function handleUpdateClassroom() {
		if (!editingClassroom.room_number) {
			toast.error('กรุณาระบุเลขห้อง');
			return;
		}
		isSubmitting = true;
		try {
			await updateClassroom(editingClassroom.id, {
				room_number: editingClassroom.room_number,
				study_plan_version_id: editingClassroom.study_plan_version_id || undefined,
				capacity: editingClassroom.capacity,
				advisors: editingAdvisors
			});
			toast.success('บันทึกข้อมูลสำเร็จ');
			showEditDialog = false;
			await fetchClassrooms();
		} catch (error) {
			console.error(error);
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			isSubmitting = false;
		}
	}

	onMount(loadInitData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

{#snippet advisorEditor(list: AdvisorRow[], setList: (next: AdvisorRow[]) => void)}
	<div class="space-y-2">
		<Label>ครูที่ปรึกษา</Label>
		<div class="flex gap-2">
			<Popover.Root bind:open={staffPickerOpen}>
				<Popover.Trigger class="flex-1">
					<Button
						variant="outline"
						role="combobox"
						aria-expanded={staffPickerOpen}
						class="w-full justify-between font-normal"
					>
						<span class="truncate">
							{addStaffId ? staffName(addStaffId) : 'เลือกครู'}
						</span>
						<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
					</Button>
				</Popover.Trigger>
				<Popover.Content class="w-[--bits-popover-trigger-width] p-0">
					<Command.Root>
						<Command.Input placeholder="ค้นหาครู..." />
						<Command.Empty>ไม่พบครู</Command.Empty>
						<Command.Group class="max-h-[280px] overflow-y-auto">
							{#each staffList.filter((s) => !list.some((a) => a.user_id === s.id)) as staff (staff.id)}
								<Command.Item
									value={`${staff.title ?? ''}${staff.name}`}
									onSelect={() => {
										addStaffId = staff.id;
										staffPickerOpen = false;
									}}
								>
									<Check
										class="mr-2 h-4 w-4 {addStaffId === staff.id ? 'opacity-100' : 'opacity-0'}"
									/>
									{staff.title ?? ''}{staff.name}
								</Command.Item>
							{/each}
						</Command.Group>
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
			<Select.Root type="single" bind:value={addRole}>
				<Select.Trigger class="w-[120px]">
					{addRole === 'primary' ? 'ครูหลัก' : 'ครูร่วม'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="primary">ครูหลัก</Select.Item>
					<Select.Item value="secondary">ครูร่วม</Select.Item>
				</Select.Content>
			</Select.Root>
			<Button
				type="button"
				variant="outline"
				onclick={() => setList(addAdvisor(list))}
				disabled={!addStaffId}
			>
				เพิ่ม
			</Button>
		</div>

		{#if list.length === 0}
			<p class="text-[11px] text-muted-foreground">ยังไม่มีครูที่ปรึกษา</p>
		{:else}
			<div class="flex flex-wrap gap-1.5">
				{#each list as a (a.user_id)}
					<Badge variant={a.role === 'primary' ? 'default' : 'secondary'} class="gap-1 pr-1">
						<button
							type="button"
							class="cursor-pointer hover:underline"
							onclick={() => setList(toggleRole(list, a.user_id))}
							title="คลิกเพื่อสลับ ครูหลัก ↔ ครูร่วม"
						>
							{a.role === 'primary' ? '⭐' : ''}
							{staffName(a.user_id)}
						</button>
						<button
							type="button"
							class="ml-1 rounded hover:bg-destructive/20 p-0.5"
							onclick={() => setList(removeAdvisor(list, a.user_id))}
							aria-label="ลบ"
						>
							<Trash2 class="h-3 w-3" />
						</button>
					</Badge>
				{/each}
			</div>
			<p class="text-[10px] text-muted-foreground">
				⭐ = ครูที่ปรึกษาหลัก — คลิกชื่อเพื่อสลับ หลัก ↔ ร่วม
			</p>
		{/if}
	</div>
{/snippet}

<div class="space-y-6">
	<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<School class="w-8 h-8" />
				จัดการห้องเรียน
			</h2>
			<p class="text-muted-foreground mt-1">สร้างห้องเรียนและกำหนดครูที่ปรึกษา</p>
		</div>
		<Button onclick={handleOpenCreate} class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			สร้างห้องเรียนใหม่
		</Button>
	</div>

	<Card.Root>
		<Card.Content class="pt-6">
			<div class="flex flex-col gap-4 md:flex-row md:items-end">
				<div class="grid w-full max-w-sm gap-1.5">
					<Label>ปีการศึกษา</Label>
					<Select.Root type="single" bind:value={selectedYearId} onValueChange={fetchClassrooms}>
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
			</div>
		</Card.Content>
	</Card.Root>

	{#if loading}
		<div class="flex h-40 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="rounded-md border bg-card">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>ระดับชั้น</Table.Head>
						<Table.Head>ชื่อห้อง</Table.Head>
						<Table.Head>จำนวนนักเรียน</Table.Head>
						<Table.Head>รับได้</Table.Head>
						<Table.Head>ครูที่ปรึกษา</Table.Head>
						<Table.Head class="text-right">จัดการ</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each classrooms as room (room.id)}
						<Table.Row>
							<Table.Cell class="font-medium">
								<Badge variant="outline">{room.grade_level_name}</Badge>
							</Table.Cell>
							<Table.Cell>
								<div class="flex flex-col">
									<span class="font-bold">{room.name}</span>
									<span class="text-xs text-muted-foreground">Code: {room.code}</span>
								</div>
							</Table.Cell>
							<Table.Cell>
								<div class="flex items-center gap-2">
									<Users class="h-4 w-4 text-muted-foreground" />
									<span>{room.student_count || 0} คน</span>
								</div>
							</Table.Cell>
							<Table.Cell>
								<span class="text-sm">{room.capacity ?? 40} คน</span>
							</Table.Cell>
							<Table.Cell>
								{#if room.advisors && room.advisors.length > 0}
									<div class="flex flex-wrap gap-1">
										{#each room.advisors as a (a.user_id)}
											<Badge
												variant={a.role === 'primary' ? 'default' : 'secondary'}
												class="font-normal"
											>
												{a.role === 'primary' ? '⭐ ' : ''}{a.name}
											</Badge>
										{/each}
									</div>
								{:else}
									<span class="text-muted-foreground text-sm">- ไม่ระบุ -</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-right">
								<Button variant="ghost" size="sm" onclick={() => handleOpenEdit(room)}>
									<Pencil class="h-4 w-4" />
								</Button>
							</Table.Cell>
						</Table.Row>
					{/each}
					{#if classrooms.length === 0}
						<Table.Row>
							<Table.Cell colspan={6} class="h-32 text-center text-muted-foreground">
								ไม่พบห้องเรียนในปีการศึกษานี้
							</Table.Cell>
						</Table.Row>
					{/if}
				</Table.Body>
			</Table.Root>
		</div>
	{/if}

	<!-- Create Dialog -->
	<Dialog.Root bind:open={showCreateDialog}>
		<Dialog.Content class="sm:max-w-[560px]">
			<Dialog.Header>
				<Dialog.Title>สร้างห้องเรียนใหม่</Dialog.Title>
				<Dialog.Description>
					เพิ่มห้องเรียนในปีการศึกษา {structure.years.find((y) => y.id === selectedYearId)?.name}
				</Dialog.Description>
			</Dialog.Header>

			<div class="grid gap-4 py-4">
				<div class="grid grid-cols-2 gap-4">
					<div class="grid gap-2">
						<Label>ระดับชั้น <span class="text-red-500">*</span></Label>
						<Select.Root type="single" bind:value={newClassroom.grade_level_id}>
							<Select.Trigger class="w-full">
								{structure.levels.find((l) => l.id === newClassroom.grade_level_id)?.name ||
									'เลือกชั้น'}
							</Select.Trigger>
							<Select.Content>
								{#each structure.levels.filter( (l) => activeLevelIds.includes(l.id) ) as level (level.id)}
									<Select.Item value={level.id}>{level.name} ({level.short_name})</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label>ชื่อห้อง/เลขห้อง <span class="text-red-500">*</span></Label>
						<Input placeholder="เช่น 1, 2, EP, Gifted" bind:value={newClassroom.room_number} />
					</div>
				</div>

				<div class="grid gap-2">
					<Label>จำนวนที่รับ (คน)</Label>
					<Input type="number" min="1" placeholder="40" bind:value={newClassroom.capacity} />
				</div>

				{@render advisorEditor(newAdvisors, (next) => (newAdvisors = next))}

				<div class="grid gap-2">
					<Label>หลักสูตรสถานศึกษา (เวอร์ชัน) <span class="text-red-500">*</span></Label>
					<Select.Root type="single" bind:value={newClassroom.study_plan_version_id}>
						<Select.Trigger class="w-full">
							{(() => {
								const v = filteredStudyPlanVersions.find(
									(v) => v.id === newClassroom.study_plan_version_id
								);
								return v ? `${v.study_plan_name_th || ''} - ${v.version_name}` : 'เลือกหลักสูตร';
							})()}
						</Select.Trigger>
						<Select.Content>
							{#each filteredStudyPlanVersions as version (version.id)}
								<Select.Item value={version.id}>
									{version.study_plan_name_th || 'หลักสูตร'} - {version.version_name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
					<p class="text-xs text-muted-foreground">
						💡 หลักสูตรจะใช้สำหรับสร้างรายวิชาอัตโนมัติในภายหลัง
					</p>
				</div>

				<div class="bg-muted/50 p-3 rounded-md text-sm text-muted-foreground">
					<p>
						ระบบจะสร้างชื่อห้องอัตโนมัติ เช่น <strong>ม.1/1</strong> หรือ <strong>ม.1/EP</strong>
					</p>
				</div>
			</div>

			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showCreateDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleCreateClassroom} disabled={isSubmitting}>
					{#if isSubmitting}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
					บันทึก
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Edit Dialog -->
	<Dialog.Root bind:open={showEditDialog}>
		<Dialog.Content class="sm:max-w-[560px]">
			<Dialog.Header>
				<Dialog.Title>แก้ไขข้อมูลห้องเรียน</Dialog.Title>
				<Dialog.Description>แก้ไขหมายเลขห้อง ครูที่ปรึกษา หรือหลักสูตร</Dialog.Description>
			</Dialog.Header>

			<div class="grid gap-4 py-4">
				<div class="grid gap-2">
					<Label>ชื่อห้อง/เลขห้อง <span class="text-red-500">*</span></Label>
					<Input placeholder="เช่น 1, 2, EP, Gifted" bind:value={editingClassroom.room_number} />
					<p class="text-xs text-muted-foreground">
						⚠️ การเปลี่ยนเลขห้องจะทำให้ รหัสห้องและชื่อห้องเปลี่ยนไปด้วย
					</p>
				</div>

				<div class="grid gap-2">
					<Label>จำนวนที่รับ (คน)</Label>
					<Input type="number" min="1" placeholder="40" bind:value={editingClassroom.capacity} />
				</div>

				{@render advisorEditor(editingAdvisors, (next) => (editingAdvisors = next))}

				<div class="grid gap-2">
					<Label>หลักสูตรสถานศึกษา (เวอร์ชัน) <span class="text-red-500">*</span></Label>
					<Select.Root type="single" bind:value={editingClassroom.study_plan_version_id}>
						<Select.Trigger class="w-full">
							{(() => {
								const v = filteredStudyPlanVersions.find(
									(v) => v.id === editingClassroom.study_plan_version_id
								);
								return v ? `${v.study_plan_name_th || ''} - ${v.version_name}` : 'เลือกหลักสูตร';
							})()}
						</Select.Trigger>
						<Select.Content>
							{#each filteredStudyPlanVersions as version (version.id)}
								<Select.Item value={version.id}>
									{version.study_plan_name_th || 'หลักสูตร'} - {version.version_name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
					<p class="text-xs text-muted-foreground">
						💡 หลักสูตรจะใช้สำหรับสร้างรายวิชาอัตโนมัติในภายหลัง
					</p>
				</div>
			</div>

			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showEditDialog = false)}>ยกเลิก</Button>
				<Button onclick={handleUpdateClassroom} disabled={isSubmitting}>
					{#if isSubmitting}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
					บันทึกการแก้ไข
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
