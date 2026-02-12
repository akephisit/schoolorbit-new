<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Checkbox from '$lib/components/ui/checkbox';
	import {
		listSubjectConstraints,
		updateSubjectConstraints,
		listPeriods,
		DAY_OPTIONS,
		type SubjectConstraintView,
		type Period
	} from '$lib/api/scheduling';
	import { Loader2, Pencil, Clock, MapPin, Layers } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	// Constants
	const TIME_PREFERENCES = [
		{ value: 'ANYTIME', label: 'เวลาใดก็ได้ (Anytime)' },
		{ value: 'MORNING', label: 'ช่วงเช้า (Morning)' },
		{ value: 'AFTERNOON', label: 'ช่วงบ่าย (Afternoon)' }
	];

	const ROOM_TYPES = [
		{ value: 'STANDARD', label: 'ห้องเรียนปกติ' },
		{ value: 'LAB_SCIENCE', label: 'ห้องวิทยาศาสตร์' },
		{ value: 'LAB_COMPUTER', label: 'ห้องคอมพิวเตอร์' },
		{ value: 'MUSIC', label: 'ห้องดนตรี' },
		{ value: 'ART', label: 'ห้องศิลปะ' },
		{ value: 'GYM', label: 'สนาม/โรงยิม' }
	];

	// State
	let subjects: SubjectConstraintView[] = [];
	let periods: Period[] = [];
	let loading = true;
	let searchTerm = '';

	// Edit State
	let showDialog = false;
	let selectedSubject: SubjectConstraintView | null = null;
	let saving = false;

	// Form Data
	let minConsecutive = 1;
	let maxConsecutive = 2;
	let preferredTime = 'ANYTIME';
	let requiredRoomType = '';
	let periodsPerWeek = 2;
	let selectedPeriodIds: string[] = [];
	let selectedDays: string[] = [];

	onMount(async () => {
		await Promise.all([loadData(), loadPeriods()]);
	});

	async function loadData() {
		loading = true;
		try {
			const res = await listSubjectConstraints();
			subjects = res.data || [];
		} catch (err) {
			console.error(err);
			toast.error('โหลดข้อมูลรายวิชาไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadPeriods() {
		try {
			const res = await listPeriods();
			periods = (res.data || []).filter((p) => p.type === 'TEACHING'); // Only teaching periods
		} catch (err) {
			console.error('Failed to load periods:', err);
		}
	}

	function openEdit(subject: SubjectConstraintView) {
		selectedSubject = subject;
		minConsecutive = subject.min_consecutive_periods || 1;
		maxConsecutive = subject.max_consecutive_periods || 2;
		preferredTime = subject.preferred_time_of_day || 'ANYTIME';
		requiredRoomType = subject.required_room_type || '';
		periodsPerWeek = subject.periods_per_week || 2;
		selectedPeriodIds = subject.allowed_period_ids || [];
		selectedDays = subject.allowed_days || [];

		showDialog = true;
	}

	async function handleSave() {
		if (!selectedSubject) return;
		saving = true;

		try {
			await updateSubjectConstraints(selectedSubject.id, {
				min_consecutive_periods: minConsecutive,
				max_consecutive_periods: maxConsecutive,
				preferred_time_of_day: preferredTime,
				required_room_type: requiredRoomType ? requiredRoomType : undefined,
				periods_per_week: periodsPerWeek,
				allowed_period_ids: selectedPeriodIds.length > 0 ? selectedPeriodIds : null,
				allowed_days: selectedDays.length > 0 ? selectedDays : null
			});
			toast.success('บันทึกข้อมูลเรียบร้อย');
			showDialog = false;
			await loadData(); // Reload
		} catch (err) {
			console.error(err);
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	$: filteredSubjects = subjects.filter(
		(s) =>
			s.code.toLowerCase().includes(searchTerm.toLowerCase()) ||
			s.name.toLowerCase().includes(searchTerm.toLowerCase())
	);
</script>

<div class="space-y-4">
	<div class="flex justify-between items-center">
		<Input placeholder="ค้นหารายวิชา (รหัส/ชื่อ)..." class="max-w-sm" bind:value={searchTerm} />
		<Button variant="outline" onclick={loadData}>
			<Loader2 class={loading ? 'animate-spin mr-2 h-4 w-4' : 'mr-2 h-4 w-4'} />
			รีเฟรช
		</Button>
	</div>

	<div class="rounded-md border">
		<table class="w-full caption-bottom text-sm text-left">
			<thead class="[&_tr]:border-b">
				<tr class="border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted">
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground w-[100px]"
						>รหัสวิชา</th
					>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ชื่อวิชา</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground"
						>คาบต่อเนื่อง (Min-Max)</th
					>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ช่วงเวลา</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground">ประเภทห้อง</th>
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground text-right"
						>ดำเนินการ</th
					>
				</tr>
			</thead>
			<tbody class="[&_tr:last-child]:border-0">
				{#each filteredSubjects as subject}
					<tr class="border-b transition-colors hover:bg-muted/50">
						<td class="p-4 align-middle font-medium">{subject.code}</td>
						<td class="p-4 align-middle">{subject.name}</td>
						<td class="p-4 align-middle">
							<div class="flex items-center gap-1">
								<Layers class="h-3 w-3 text-muted-foreground" />
								<span
									>{subject.min_consecutive_periods || 1} - {subject.max_consecutive_periods ||
										2}</span
								>
							</div>
						</td>
						<td class="p-4 align-middle">
							{#if subject.preferred_time_of_day === 'MORNING'}
								<span class="text-xs bg-orange-100 text-orange-800 px-2 py-1 rounded">เช้า ☀️</span>
							{:else if subject.preferred_time_of_day === 'AFTERNOON'}
								<span class="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">บ่าย 🌙</span>
							{:else}
								<span class="text-xs text-muted-foreground">เวลาใดก็ได้</span>
							{/if}
						</td>
						<td class="p-4 align-middle">
							{#if subject.required_room_type}
								<div class="flex items-center gap-1 text-purple-600 font-medium text-xs">
									<MapPin class="h-3 w-3" />
									{subject.required_room_type}
								</div>
							{:else}
								<span class="text-xs text-muted-foreground">-</span>
							{/if}
						</td>
						<td class="p-4 align-middle text-right">
							<Button variant="ghost" size="sm" onclick={() => openEdit(subject)}>
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
	<Dialog.Content class="sm:max-w-[500px]">
		<Dialog.Header>
			<Dialog.Title>ตั้งค่าวิชา: {selectedSubject?.code} {selectedSubject?.name}</Dialog.Title>
			<Dialog.Description>กำหนดเงื่อนไขการเรียนการสอนและสถานที่</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-4 py-4">
			<!-- Consecutive -->
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label>เรียนต่อเนื่องขั้นต่ำ (คาบ)</Label>
					<Input type="number" min="1" max="4" bind:value={minConsecutive} />
				</div>
				<div class="space-y-2">
					<Label>เรียนต่อเนื่องสูงสุด (คาบ)</Label>
					<Input type="number" min="1" max="4" bind:value={maxConsecutive} />
				</div>
			</div>

			<!-- Time Preference -->
			<div class="space-y-2">
				<Label>ช่วงเวลาที่ต้องการ</Label>
				<select
					class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
					bind:value={preferredTime}
				>
					{#each TIME_PREFERENCES as time}
						<option value={time.value}>{time.label}</option>
					{/each}
				</select>
			</div>

			<!-- Allowed Period IDs -->
			<div class="space-y-2">
				<Label>คาบที่อนุญาต (ไม่เลือก = ทุกคาบ)</Label>
				<div class="grid grid-cols-2 gap-2 max-h-48 overflow-y-auto p-2 border rounded-md">
					{#each periods as period}
						<label class="flex items-center space-x-2 text-sm cursor-pointer">
							<input
								type="checkbox"
								value={period.id}
								checked={selectedPeriodIds.includes(period.id)}
								on:change={(e) => {
									if (e.currentTarget.checked) {
										selectedPeriodIds = [...selectedPeriodIds, period.id];
									} else {
										selectedPeriodIds = selectedPeriodIds.filter((id) => id !== period.id);
									}
								}}
								class="h-4 w-4 rounded border-gray-300"
							/>
							<span>{period.name} ({period.start_time})</span>
						</label>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					เลือกคาบที่วิชานี้สามารถจัดได้ (ว่างไว้ = ทุกคาบ)
				</p>
			</div>

			<!-- Allowed Days -->
			<div class="space-y-2">
				<Label>วันที่อนุญาต (ไม่เลือก = ทุกวัน)</Label>
				<div class="grid grid-cols-2 gap-2">
					{#each DAY_OPTIONS as day}
						<label class="flex items-center space-x-2 text-sm cursor-pointer">
							<input
								type="checkbox"
								value={day.value}
								checked={selectedDays.includes(day.value)}
								on:change={(e) => {
									if (e.currentTarget.checked) {
										selectedDays = [...selectedDays, day.value];
									} else {
										selectedDays = selectedDays.filter((d) => d !== day.value);
									}
								}}
								class="h-4 w-4 rounded border-gray-300"
							/>
							<span>{day.label}</span>
						</label>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					เลือกวันที่วิชานี้สามารถจัดได้ (ว่างไว้ = ทุกวัน)
				</p>
			</div>

			<!-- Room Type -->
			<div class="space-y-2">
				<Label>ประเภทห้องที่ต้องการ</Label>
				<select
					class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
					bind:value={requiredRoomType}
				>
					<option value="">-- ไม่ระบุ (ห้องเรียนปกติ) --</option>
					{#each ROOM_TYPES as room}
						<option value={room.value}>{room.label}</option>
					{/each}
				</select>
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
