<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Dialog from '$lib/components/ui/dialog';
	import {
		listSubjectConstraints,
		updateSubjectConstraints,
		listPeriods,
		DAY_OPTIONS,
		type SubjectConstraintView,
		type Period
	} from '$lib/api/scheduling';
	import { Loader2, Pencil, Layers } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

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
	let periodsPerWeek = 2;
	let selectedPeriodIds: string[] = [];
	let selectedDays: string[] = [];
	let showOnlyTeachingPeriods = true; // Default: filter out breaks/activities

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
			periods = res.data || []; // Load all period types
		} catch (err) {
			console.error('Failed to load periods:', err);
		}
	}

	// Filter periods based on type toggle
	$: filteredPeriods = showOnlyTeachingPeriods
		? periods.filter((p) => p.type === 'TEACHING')
		: periods;

	function openEdit(subject: SubjectConstraintView) {
		selectedSubject = subject;
		minConsecutive = subject.min_consecutive_periods || 1;
		maxConsecutive = subject.max_consecutive_periods || 2;
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

	// Calculate available slots based on current constraints
	$: availableSlots = (() => {
		if (!selectedSubject) return 0;

		let count = filteredPeriods.length * 5; // Default: filtered periods × 5 days (Mon-Fri)

		// Apply day filter
		if (selectedDays.length > 0) {
			count = filteredPeriods.length * selectedDays.length;
		}

		// Apply period filter
		if (selectedPeriodIds.length > 0) {
			const dayCount = selectedDays.length > 0 ? selectedDays.length : 5;
			count = selectedPeriodIds.length * dayCount;
		}

		return count;
	})();

	// Check if constraints are too restrictive
	$: constraintWarning = (() => {
		if (!selectedSubject) return '';

		const needed = periodsPerWeek || 2;
		if (availableSlots < needed) {
			return `⚠️ คำเตือน: มีช่องว่างเพียง ${availableSlots} ช่อง แต่ต้องการ ${needed} คาบต่อสัปดาห์`;
		}
		if (availableSlots < needed * 1.5) {
			return `⚡ หมายเหตุ: มีช่องว่างเพียง ${availableSlots} ช่อง สำหรับ ${needed} คาบ (อาจจัดตารางลำบาก)`;
		}
		return '';
	})();

	// Reminder to check instructor availability
	$: instructorHint =
		selectedPeriodIds.length > 0 || selectedDays.length > 0
			? '💡 หมายเหตุ: อย่าลืมตรวจสอบว่าครูผู้สอนว่างในช่วงเวลาที่เลือกด้วย'
			: '';

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
					<th class="h-12 px-4 align-middle font-medium text-muted-foreground text-right"
						>ดำเนินการ</th
					>
				</tr>
			</thead>
			<tbody class="[&_tr:last-child]:border-0">
				{#each filteredSubjects as subject (subject.id)}
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

			<!-- Allowed Period IDs -->
			<div class="space-y-2">
				<Label>คาบที่อนุญาต (ไม่เลือก = ทุกคาบ)</Label>
				<div class="grid grid-cols-2 gap-2 max-h-48 overflow-y-auto p-2 border rounded-md">
					{#each periods as period (period.id)}
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
					{#each DAY_OPTIONS as day (day.value)}
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

			<!-- Room Assignment Info -->
			<div class="rounded-lg border bg-slate-50 dark:bg-slate-900 p-3">
				<p class="text-sm text-slate-700 dark:text-slate-300">
					📍 <strong>หมายเหตุ:</strong> ห้องเรียนจะถูกกำหนดตามห้องประจำของครูผู้สอน (ตั้งค่าได้ที่แท็บ
					"ข้อมูลครู")
				</p>
			</div>

			<!-- Preview / Warning -->
			{#if selectedPeriodIds.length > 0 || selectedDays.length > 0}
				<div class="rounded-lg border bg-blue-50 dark:bg-blue-950 p-3">
					<div class="flex items-start gap-2">
						<div class="text-blue-600 dark:text-blue-400 mt-0.5">
							<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
								></path>
							</svg>
						</div>
						<div class="flex-1">
							<p class="text-sm font-medium text-blue-900 dark:text-blue-100">
								ช่วงเวลาที่สามารถจัดได้: {availableSlots} ช่อง
							</p>
							<p class="text-xs text-blue-700 dark:text-blue-300 mt-1">
								ต้องการ {periodsPerWeek || 2} คาบต่อสัปดาห์
							</p>
						</div>
					</div>
				</div>
			{/if}

			{#if constraintWarning}
				<div
					class="rounded-lg border bg-yellow-50 dark:bg-yellow-950 border-yellow-200 dark:border-yellow-800 p-3"
				>
					<p class="text-sm text-yellow-800 dark:text-yellow-200">
						{constraintWarning}
					</p>
				</div>
			{/if}

			{#if instructorHint}
				<div
					class="rounded-lg border bg-purple-50 dark:bg-purple-950 border-purple-200 dark:border-purple-800 p-3"
				>
					<p class="text-sm text-purple-800 dark:text-purple-200 flex items-center gap-2">
						<span>{instructorHint}</span>
					</p>
				</div>
			{/if}
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
