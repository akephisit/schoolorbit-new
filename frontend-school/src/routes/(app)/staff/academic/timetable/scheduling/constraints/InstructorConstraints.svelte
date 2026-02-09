<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import {
		listInstructorConstraints,
		updateInstructorConstraints,
		type InstructorConstraintView
	} from '$lib/api/scheduling';
	import { Loader2, Pencil, CalendarClock, Briefcase, User } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	// Constants
	const DAYS = ['MON', 'TUE', 'WED', 'THU', 'FRI'];
	const PERIODS = 8;

	// Mock Rooms (TODO: Fetch from API)
	const ROOMS = [
		{ value: 'uuid-1', label: 'Room 101' },
		{ value: 'uuid-2', label: 'Science Lab 1' },
		{ value: 'uuid-3', label: 'Computer Lab' },
		{ value: 'uuid-4', label: 'Music Room' },
		{ value: 'uuid-5', label: 'Gym' }
	];

	// State
	let instructors: InstructorConstraintView[] = [];
	let loading = true;
	let searchTerm = '';

	// Editing State
	let showDialog = false;
	let selectedInstructor: InstructorConstraintView | null = null;
	let saving = false;

	// Form Data
	let maxPeriods = 7;
	let assignedRoomId = '';
	// Grid: [DayIndex][PeriodIndex] -> true/false (true = busy/unavailable)
	let busyGrid: boolean[][] = [];

	onMount(async () => {
		await loadData();
	});

	async function loadData() {
		loading = true;
		try {
			const res = await listInstructorConstraints();
			instructors = res.data || [];
		} catch (err) {
			console.error(err);
			toast.error('โหลดข้อมูลครูไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	function openEdit(instructor: InstructorConstraintView) {
		selectedInstructor = instructor;
		maxPeriods = instructor.max_periods_per_day || 7;
		assignedRoomId = instructor.assigned_room_id || '';

		// Init Grid (Default Available)
		busyGrid = Array(5)
			.fill(null)
			.map(() => Array(PERIODS).fill(false));

		// Parse Unavailable Slots
		if (instructor.hard_unavailable_slots && Array.isArray(instructor.hard_unavailable_slots)) {
			// Example format from DB: [{ day_index: 0, period_index: 0 }, ...]
			instructor.hard_unavailable_slots.forEach((slot: any) => {
				if (slot.day_index !== undefined && slot.period_index !== undefined) {
					if (
						busyGrid[slot.day_index] &&
						busyGrid[slot.day_index][slot.period_index] !== undefined
					) {
						busyGrid[slot.day_index][slot.period_index] = true;
					}
				}
			});
		}

		showDialog = true;
	}

	function toggleSlot(dayIndex: number, periodIndex: number) {
		// Toggle state
		busyGrid[dayIndex][periodIndex] = !busyGrid[dayIndex][periodIndex];
	}

	async function handleSave() {
		if (!selectedInstructor) return;
		saving = true;

		// Convert grid back to JSON
		const unavailableSlots = [];
		for (let d = 0; d < 5; d++) {
			for (let p = 0; p < PERIODS; p++) {
				if (busyGrid[d][p]) {
					unavailableSlots.push({ day_index: d, period_index: p, day: DAYS[d] });
				}
			}
		}

		try {
			await updateInstructorConstraints(selectedInstructor.id, {
				max_periods_per_day: maxPeriods,
				assigned_room_id: assignedRoomId ? assignedRoomId : undefined, // Handle empty string
				hard_unavailable_slots: unavailableSlots
			});
			toast.success('บันทึกข้อมูลเรียบร้อย');
			showDialog = false;
			loadData(); // Reload
		} catch (err) {
			console.error(err);
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	$: filteredInstructors = instructors.filter(
		(i) =>
			i.first_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
			i.last_name.toLowerCase().includes(searchTerm.toLowerCase())
	);
</script>

<div class="space-y-4">
	<div class="flex justify-between items-center">
		<Input placeholder="ค้นหาครู..." class="max-w-sm" bind:value={searchTerm} />
		<Button variant="outline" onclick={loadData}>
			<Loader2 class={loading ? 'animate-spin mr-2 h-4 w-4' : 'mr-2 h-4 w-4'} />
			รีเฟรช
		</Button>
	</div>

	<div class="rounded-md border">
		<table class="w-full caption-bottom text-sm text-left">
			<thead class="[&_tr]:border-b">
				<tr class="border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted">
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[50px]">#</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ชื่อ-สกุล</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">สอนสูงสุด/วัน</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ห้องประจำ</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground text-right"
						>ดำเนินการ</th
					>
				</tr>
			</thead>
			<tbody class="[&_tr:last-child]:border-0">
				{#each filteredInstructors as instructor, i}
					<tr class="border-b transition-colors hover:bg-muted/50">
						<td class="p-4 align-middle">{i + 1}</td>
						<td class="p-4 align-middle font-medium">
							<div class="flex flex-col">
								<span>{instructor.first_name} {instructor.last_name}</span>
								{#if instructor.short_name}
									<span class="text-xs text-muted-foreground">({instructor.short_name})</span>
								{/if}
							</div>
						</td>
						<td class="p-4 align-middle">
							<span class="px-2 py-1 bg-secondary rounded-md text-xs font-medium">
								{instructor.max_periods_per_day || 7} คาบ
							</span>
						</td>
						<td class="p-4 align-middle">
							{#if instructor.assigned_room_name}
								<div class="flex items-center gap-1 text-blue-600">
									<Briefcase class="h-3 w-3" />
									{instructor.assigned_room_name}
								</div>
							{:else}
								<span class="text-muted-foreground text-xs">-</span>
							{/if}
						</td>
						<td class="p-4 align-middle text-right">
							<Button variant="ghost" size="sm" onclick={() => openEdit(instructor)}>
								<Pencil class="h-4 w-4 mr-2" />
								ตั้งค่า
							</Button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>

<!-- Edit Dialog -->
<Dialog.Root bind:open={showDialog}>
	<Dialog.Content class="sm:max-w-[700px]">
		<Dialog.Header>
			<Dialog.Title
				>ตั้งค่าเงื่อนไข: {selectedInstructor?.first_name}
				{selectedInstructor?.last_name}</Dialog.Title
			>
			<Dialog.Description>กำหนดเวลาที่ไม่ว่างและภาระงานสอน</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-6 py-4">
			<!-- Settings -->
			<div class="grid grid-cols-2 gap-4">
				<div class="grid gap-2">
					<Label for="maxPeriods">จำนวนคาบสูงสุดต่อวัน</Label>
					<Input id="maxPeriods" type="number" min="1" max="8" bind:value={maxPeriods} />
				</div>
				<div class="grid gap-2">
					<Label for="room">ห้องประจำตำแหน่ง</Label>
					<select
						id="room"
						class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
						bind:value={assignedRoomId}
					>
						<option value="">-- ไม่ระบุ --</option>
						{#each ROOMS as room}
							<option value={room.value}>{room.label}</option>
						{/each}
					</select>
				</div>
			</div>

			<!-- Availability Grid -->
			<div class="space-y-2">
				<div class="flex items-center justify-between">
					<Label>ช่วงเวลาที่ไม่ว่าง (คลิกเพื่อเปลี่ยนสี)</Label>
					<div class="flex gap-4 text-xs">
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-white border rounded"></div>
							<span>ว่าง (Available)</span>
						</div>
						<div class="flex items-center gap-1">
							<div class="w-3 h-3 bg-red-100 border border-red-200 rounded"></div>
							<span>ห้ามจัด (Busy)</span>
						</div>
					</div>
				</div>

				<div class="border rounded-md p-2 overflow-x-auto">
					<div class="min-w-[500px]">
						<!-- Header -->
						<div class="grid grid-cols-[60px_repeat(8,1fr)] gap-1 mb-1">
							<div class="font-bold text-xs text-center p-2">วัน</div>
							{#each Array(PERIODS) as _, p}
								<div class="font-bold text-xs text-center p-2 bg-muted rounded">P{p + 1}</div>
							{/each}
						</div>

						<!-- Rows -->
						{#each DAYS as day, d}
							<div class="grid grid-cols-[60px_repeat(8,1fr)] gap-1 mb-1">
								<div class="font-bold text-xs flex items-center justify-center bg-muted rounded">
									{day}
								</div>
								{#each Array(PERIODS) as _, p}
									<button
										class="h-8 rounded border transition-colors text-xs flex items-center justify-center
											{busyGrid[d] && busyGrid[d][p]
											? 'bg-red-100 border-red-200 text-red-700 hover:bg-red-200'
											: 'bg-white hover:bg-slate-50'}"
										onclick={() => toggleSlot(d, p)}
									>
										{busyGrid[d] && busyGrid[d][p] ? 'BUSY' : ''}
									</button>
								{/each}
							</div>
						{/each}
					</div>
				</div>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleSave} disabled={saving}>
				{#if saving}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					บันทึก...
				{:else}
					บันทึกการเปลี่ยนแปลง
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
