<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAcademicStructure,
		listClassrooms,
		createClassroom,
		type AcademicStructureData,
		type Classroom
	} from '$lib/api/academic';
	import { listStaff, type StaffListItem } from '$lib/api/staff';
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
	import Layers from 'lucide-svelte/icons/layers';
	import Filter from 'lucide-svelte/icons/filter';
	import Plus from 'lucide-svelte/icons/plus';
	import Users from 'lucide-svelte/icons/users';

	let loading = true;
	let structure: AcademicStructureData = { years: [], semesters: [], levels: [] };
	let classrooms: Classroom[] = [];
	let staffList: StaffListItem[] = [];

	let showCreateDialog = false;
	let isSubmitting = false;

	// Filter State
	let selectedYearId = '';

	// New Classroom Form
	let newClassroom = {
		academic_year_id: '',
		grade_level_id: '',
		room_number: '',
		advisor_id: '',
		co_advisor_id: ''
	};

	async function loadInitData() {
		try {
			loading = true;
			const [structureRes, staffRes] = await Promise.all([
				getAcademicStructure(),
				listStaff({ page: 1, page_size: 100 }) // Fetch teachers for dropdown
			]);
			structure = structureRes.data;
			staffList = staffRes.data;

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
			const res = await listClassrooms({ year_id: selectedYearId });
			classrooms = res.data;
		} catch (error) {
			console.error(error);
			toast.error('โหลดข้อมูลห้องเรียนไม่สำเร็จ');
		}
	}

	async function handleCreateClassroom() {
		if (!newClassroom.academic_year_id || !newClassroom.grade_level_id || !newClassroom.room_number) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็นให้ครบถ้วน');
			return;
		}

		isSubmitting = true;
		try {
			// Convert empty strings to undefined for optional fields
			const payload = {
				...newClassroom,
				advisor_id: newClassroom.advisor_id || undefined,
				co_advisor_id: newClassroom.co_advisor_id || undefined
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

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
		<div>
			<h2 class="text-2xl font-bold tracking-tight">จัดการห้องเรียน</h2>
			<p class="text-muted-foreground">สร้างห้องเรียนและกำหนดครูที่ปรึกษา</p>
		</div>
		<Button onclick={() => (showCreateDialog = true)}>
			<Plus class="mr-2 h-4 w-4" />
			สร้างห้องเรียนใหม่
		</Button>
	</div>

	<!-- Filters -->
	<Card.Root>
		<Card.Content class="pt-6">
			<div class="flex flex-col gap-4 md:flex-row md:items-end">
				<div class="grid w-full max-w-sm gap-1.5">
					<Label>ปีการศึกษา</Label>
					<select
						class="flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
						bind:value={selectedYearId}
						onchange={fetchClassrooms}
					>
						{#each structure.years as year}
							<option value={year.id}>{year.name} {year.is_active ? '(ปัจจุบัน)' : ''}</option>
						{/each}
					</select>
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
								<Button variant="ghost" size="sm">แก้ไข</Button>
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
						<select
							class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
							bind:value={newClassroom.grade_level_id}
						>
							<option value="" disabled>เลือกชั้น</option>
							{#each structure.levels as level}
								<option value={level.id}>{level.name} ({level.short_name})</option>
							{/each}
						</select>
					</div>
					<div class="grid gap-2">
						<Label>ชื่อห้อง/เลขห้อง <span class="text-red-500">*</span></Label>
						<Input placeholder="เช่น 1, 2, EP, Gifted" bind:value={newClassroom.room_number} />
					</div>
				</div>

				<div class="grid gap-2">
					<Label>ครูที่ปรึกษาหลัก</Label>
					<select
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						bind:value={newClassroom.advisor_id}
					>
						<option value="">- ไม่ระบุ -</option>
						{#each staffList as staff}
							<option value={staff.id}>{staff.first_name} {staff.last_name}</option>
						{/each}
					</select>
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
