<script lang="ts">
	import { onMount } from 'svelte';

	let { data } = $props();

	import {
		getAcademicStructure,
		listClassrooms,
		createClassroom,
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
	import { Loader2, Layers, Filter, Plus, Users, School, Pencil } from 'lucide-svelte';

	let loading = $state(true);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let classrooms = $state<Classroom[]>([]);
	let staffList = $state<StaffLookupItem[]>([]);
	let activeLevelIds = $state<string[]>([]);
	let studyPlanVersions = $state<StudyPlanVersion[]>([]);

	let showCreateDialog = $state(false);
	let isSubmitting = $state(false);

	// Filter State
	let selectedYearId = $state('');

	// New Classroom Form
	let newClassroom = $state({
		academic_year_id: '',
		grade_level_id: '',
		room_number: '',
		advisor_id: '',
		co_advisor_id: '',
		study_plan_version_id: ''
	});

	async function loadInitData() {
		try {
			loading = true;
			const [structureRes, staffData, versionsRes] = await Promise.all([
				getAcademicStructure(),
				lookupStaff(), // Lookup API - only requires authentication
				listStudyPlanVersions({ active_only: true })
			]);
			structure = structureRes.data;
			staffList = staffData;
			studyPlanVersions = versionsRes.data;

			// Auto-select latest active year
			const activeYear = structure.years.find((y) => y.is_active) || structure.years[0];
			if (activeYear) {
				selectedYearId = activeYear.id;
				newClassroom.academic_year_id = activeYear.id;
			}

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

	async function handleCreateClassroom() {
		if (
			!newClassroom.academic_year_id ||
			!newClassroom.grade_level_id ||
			!newClassroom.room_number
		) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็นให้ครบถ้วน');
			return;
		}

		isSubmitting = true;
		try {
			// Convert empty strings to undefined for optional fields
			const payload = {
				...newClassroom,
				advisor_id: newClassroom.advisor_id || undefined,
				co_advisor_id: newClassroom.co_advisor_id || undefined,
				study_plan_version_id: newClassroom.study_plan_version_id || undefined
			};

			await createClassroom(payload);
			toast.success('สร้างห้องเรียนสำเร็จ');
			showCreateDialog = false;
			await fetchClassrooms();

			// Reset pertinent fields (keep year)
			newClassroom.room_number = '';
			newClassroom.advisor_id = '';
			newClassroom.co_advisor_id = '';
		} catch (error) {
			console.error(error);
			toast.error('สร้างห้องเรียนไม่สำเร็จ (ชื่อห้องซ้ำหรือข้อมูลไม่ถูกต้อง)');
		} finally {
			isSubmitting = false;
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
				<School class="w-8 h-8" />
				จัดการห้องเรียน
			</h2>
			<p class="text-muted-foreground mt-1">สร้างห้องเรียนและกำหนดครูที่ปรึกษา</p>
		</div>
		<Button onclick={() => (showCreateDialog = true)} class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			สร้างห้องเรียนใหม่
		</Button>
	</div>

	<!-- Filters -->
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
							{#each structure.years as year}
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

	<!-- Content -->
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
						<Table.Head>ครูที่ปรึกษา</Table.Head>
						<Table.Head class="text-right">จัดการ</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each classrooms as room}
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
								{#if room.advisor_name}
									{room.advisor_name}
								{:else}
									<span class="text-muted-foreground text-sm">- ไม่ระบุ -</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-right">
								<Button variant="ghost" size="sm">
									<Pencil class="h-4 w-4" />
								</Button>
							</Table.Cell>
						</Table.Row>
					{/each}
					{#if classrooms.length === 0}
						<Table.Row>
							<Table.Cell colspan={5} class="h-32 text-center text-muted-foreground">
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
		<Dialog.Content class="sm:max-w-[500px]">
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
								{#each structure.levels.filter((l) => activeLevelIds.includes(l.id)) as level}
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
					<Label>ครูที่ปรึกษาหลัก</Label>
					<Select.Root type="single" bind:value={newClassroom.advisor_id}>
						<Select.Trigger class="w-full">
							{(() => {
								const s = staffList.find((s) => s.id === newClassroom.advisor_id);
								return s ? `${s.title || ''}${s.name}` : '- ไม่ระบุ -';
							})()}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">- ไม่ระบุ -</Select.Item>
							{#each staffList as staff}
								<Select.Item value={staff.id}
									>{staff.title ? `${staff.title}` : ''}{staff.name}</Select.Item
								>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="grid gap-2">
					<Label>หลักสูตรสถานศึกษา (เวอร์ชัน)</Label>
					<Select.Root type="single" bind:value={newClassroom.study_plan_version_id}>
						<Select.Trigger class="w-full">
							{(() => {
								const v = studyPlanVersions.find(
									(v) => v.id === newClassroom.study_plan_version_id
								);
								return v
									? `${v.study_plan_name_th || ''} - ${v.version_name}`
									: '- ไม่ระบุ (เพิ่มรายวิชาด้วยตนเอง) -';
							})()}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">- ไม่ระบุ (เพิ่มรายวิชาด้วยตนเอง) -</Select.Item>
							{#each studyPlanVersions as version}
								<Select.Item value={version.id}>
									{version.study_plan_name_th || 'หลักสูตร'} - {version.version_name}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
					<p class="text-xs text-muted-foreground">
						💡 หากเลือกหลักสูตร คุณสามารถใช้ฟีเจอร์ "สร้างรายวิชาอัตโนมัติ" ได้ในภายหลัง
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
					{#if isSubmitting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					บันทึก
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
