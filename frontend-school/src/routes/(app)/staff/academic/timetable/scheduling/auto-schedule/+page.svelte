<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Loader2, Zap, Settings2, History, CalendarDays } from 'lucide-svelte';
	import type { UUID } from '$lib/types';
	import type { Classroom } from '$lib/api/academic';
	import type { SchedulingAlgorithm, CreateSchedulingJobRequest } from '$lib/api/scheduling';
	import { listClassrooms, getAcademicStructure } from '$lib/api/academic';
	import { autoScheduleTimetable } from '$lib/api/scheduling';
	import * as Select from '$lib/components/ui/select';

	let loading = $state(false);
	let submitting = $state(false);

	// Data
	let classrooms = $state<Classroom[]>([]);
	let allSemesters = $state<any[]>([]);
	let allYears = $state<any[]>([]);
	let selectedSemesterId = $state<UUID | null>(null);
	let selectedClassroomIds = $state<UUID[]>([]);

	let selectedSemester = $derived.by(() => {
		const semester = allSemesters.find((s) => s.id === selectedSemesterId);
		if (!semester) return undefined;

		const year = allYears.find((y) => y.id === semester.academic_year_id);
		const yearLabel = year ? year.year : 'N/A';

		return {
			value: selectedSemesterId,
			label: `${semester.term}/${yearLabel}`
		};
	});

	// Config
	let algorithm = $state<SchedulingAlgorithm>('BACKTRACKING');
	let forceOverwrite = $state(false);
	let allowPartial = $state(false);
	let minQualityScore = $state(95); // Detailed (High Quality)
	let timeoutSeconds = $state(300); // 5 Minutes
	let showAdvanced = $state(false);

	// Advanced Config
	let allowMultipleSessions = $state(false); // Default: Force spread (strict)

	onMount(async () => {
		// Load data in parallel for better performance
		loading = true;
		await Promise.all([loadAcademicData(), loadClassrooms()]);
		loading = false;

		// Get semester from query params if available
		const semesterId = $page.url.searchParams.get('semester_id');
		if (semesterId) {
			selectedSemesterId = semesterId;
		}
	});

	async function loadAcademicData() {
		try {
			const res = await getAcademicStructure();
			allSemesters = res.data.semesters;
			allYears = res.data.years;

			// Auto-select active semester
			if (!selectedSemesterId) {
				const active = allSemesters.find((s) => s.is_active);
				if (active) selectedSemesterId = active.id;
				else if (allSemesters.length > 0) selectedSemesterId = allSemesters[0].id;
			}
		} catch (error) {
			console.error('Failed to load academic data:', error);
		}
	}

	async function loadClassrooms() {
		try {
			const res = await listClassrooms();
			classrooms = res.data || [];
		} catch (error) {
			toast.error('โหลดข้อมูลห้องเรียนไม่สำเร็จ');
		}
	}

	function toggleClassroom(id: UUID) {
		if (selectedClassroomIds.includes(id)) {
			selectedClassroomIds = selectedClassroomIds.filter((cid) => cid !== id);
		} else {
			selectedClassroomIds = [...selectedClassroomIds, id];
		}
	}

	function selectAll() {
		selectedClassroomIds = classrooms.map((c) => c.id);
	}

	function selectNone() {
		selectedClassroomIds = [];
	}

	async function handleSubmit() {
		if (!selectedSemesterId) {
			toast.error('กรุณาเลือกภาคเรียน');
			return;
		}

		if (selectedClassroomIds.length === 0) {
			toast.error('กรุณาเลือกห้องเรียนอย่างน้อย 1 ห้อง');
			return;
		}

		submitting = true;

		try {
			const request: CreateSchedulingJobRequest = {
				academic_semester_id: selectedSemesterId,
				classroom_ids: selectedClassroomIds,
				algorithm,
				config: {
					force_overwrite: forceOverwrite,
					allow_partial: allowPartial,
					min_quality_score: minQualityScore,
					timeout_seconds: timeoutSeconds,
					allow_multiple_sessions_per_day: allowMultipleSessions
				}
			};

			const res = await autoScheduleTimetable(request);

			if (res.data?.job_id) {
				toast.success('เริ่มจัดตารางอัตโนมัติแล้ว');
				goto(`/staff/academic/timetable/scheduling/jobs/${res.data.job_id}`);
			}
		} catch (error: any) {
			console.error('Auto-schedule error:', error);
			toast.error(error.message || 'เกิดข้อผิดพลาดในการจัดตาราง');
		} finally {
			submitting = false;
		}
	}

	let selectedCount = $derived(selectedClassroomIds.length);
</script>

<div class="container mx-auto p-6 max-w-4xl">
	<div class="flex items-center justify-between mb-6">
		<div>
			<h1 class="text-3xl font-bold mb-2">จัดตารางอัตโนมัติ</h1>
			<p class="text-muted-foreground">
				ระบบจะจัดตารางสอนให้อัตโนมัติตามเงื่อนไขและความต้องการที่กำหนด
			</p>
		</div>
		<div class="flex gap-2">
			<Button
				variant="outline"
				onclick={() => goto('/staff/academic/timetable/scheduling/constraints')}
			>
				<Settings2 class="mr-2 h-4 w-4" />
				ตั้งค่าเงื่อนไข
			</Button>
			<Button variant="outline" onclick={() => goto('/staff/academic/timetable/scheduling/jobs')}>
				<History class="mr-2 h-4 w-4" />
				ประวัติการจัดตาราง
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<div class="space-y-6">
			<!-- Semester Selection -->
			<div class="flex items-center gap-4">
				<div class="w-[300px]">
					<Label class="mb-2 block">ภาคเรียน</Label>
					<Select.Root
						type="single"
						value={selectedSemesterId || undefined}
						onValueChange={(v) => {
							if (v) selectedSemesterId = v;
						}}
					>
						<Select.Trigger>
							<div class="flex items-center gap-2">
								<CalendarDays class="h-4 w-4 text-muted-foreground" />
								<span>{selectedSemester?.label || 'เลือกภาคเรียน'}</span>
							</div>
						</Select.Trigger>
						<Select.Content>
							{#each allSemesters as semester}
								{@const year = allYears.find((y) => y.id === semester.academic_year_id)}
								{@const yearLabel = year ? year.year : 'N/A'}
								<Select.Item value={semester.id} label={`${semester.term}/${yearLabel}`}>
									{semester.term}/{yearLabel}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<!-- Classroom Selection -->
			<Card class="p-6">
				<div class="mb-4">
					<h2 class="text-xl font-semibold mb-2">เลือกห้องเรียน</h2>
					<p class="text-sm text-muted-foreground mb-4">
						เลือกห้องเรียนที่ต้องการจัดตาราง ({selectedCount} ห้อง)
					</p>

					<div class="flex gap-2 mb-4">
						<Button variant="outline" size="sm" onclick={selectAll}>เลือกทั้งหมด</Button>
						<Button variant="outline" size="sm" onclick={selectNone}>ยกเลิกทั้งหมด</Button>
					</div>
				</div>

				<div class="grid grid-cols-2 md:grid-cols-4 gap-3 max-h-[400px] overflow-y-auto">
					{#each classrooms as classroom}
						<label
							class="flex items-center space-x-2 p-3 rounded-lg border cursor-pointer hover:bg-accent transition-colors"
							class:bg-accent={selectedClassroomIds.includes(classroom.id)}
						>
							<Checkbox
								checked={selectedClassroomIds.includes(classroom.id)}
								onCheckedChange={() => toggleClassroom(classroom.id)}
							/>
							<span class="font-medium">{classroom.name}</span>
						</label>
					{/each}
				</div>
			</Card>

			<!-- Conditions Settings -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-4 flex items-center gap-2">
					<Settings2 class="h-5 w-5" />
					เงื่อนไขการจัดตาราง
				</h2>

				<div class="space-y-5">
					<!-- Spread Days -->
					<div class="flex items-start space-x-3">
						<Checkbox
							id="spreadDays"
							checked={!allowMultipleSessions}
							onCheckedChange={(c) => (allowMultipleSessions = !c)}
						/>
						<div class="grid gap-1.5 leading-none">
							<Label for="spreadDays" class="text-base font-medium leading-none cursor-pointer">
								บังคับเรียนคนละวัน (Force Spread Days)
							</Label>
							<p class="text-sm text-muted-foreground">
								ห้ามเรียนวิชาเดียวกันซ้ำในวันเดียว (ยกเว้นคาบต่อเนื่อง) เพื่อกระจายภาระการเรียน
							</p>
						</div>
					</div>

					<div class="border-t my-4"></div>

					<!-- Overwrite -->
					<div class="flex items-start space-x-3">
						<Checkbox id="overwrite" bind:checked={forceOverwrite} />
						<div class="grid gap-1.5 leading-none">
							<Label for="overwrite" class="text-base font-medium leading-none cursor-pointer">
								เขียนทับตารางเดิม (Force Overwrite)
							</Label>
							<p class="text-sm text-muted-foreground">
								หากมีตารางเรียนอยู่แล้ว จะทำการลบและจัดใหม่ทั้งหมด (ถ้าไม่เลือก จะจัดเฉพาะช่องว่าง)
							</p>
						</div>
					</div>

					<!-- Partial -->
					<div class="flex items-start space-x-3">
						<Checkbox id="partial" bind:checked={allowPartial} />
						<div class="grid gap-1.5 leading-none">
							<Label for="partial" class="text-base font-medium leading-none cursor-pointer">
								อนุญาตให้จัดไม่ครบ (Allow Partial)
							</Label>
							<p class="text-sm text-muted-foreground">
								ให้ระบบจัดตารางเท่าที่ทำได้ แม้จะมีบางวิชาที่หาลงไม่ได้ (ถ้าไม่เลือก ระบบจะฟ้อง
								Error หากจัดไม่ครบ)
							</p>
						</div>
					</div>
				</div>

				<!-- Hidden Technical Settings (Defaults) -->
				{#if showAdvanced}
					<!-- Only show if explicitly toggled (for debugging), otherwise hidden as requested -->
					<div class="mt-6 pt-6 border-t space-y-4">
						<h3 class="font-medium text-muted-foreground">การตั้งค่าขั้นสูง (Technical)</h3>
						<div>
							<Label>Algorithm</Label>
							<select bind:value={algorithm} class="w-full mt-1 border rounded p-2 text-sm">
								<option value="BACKTRACKING">Backtracking</option>
								<option value="GREEDY">Greedy</option>
							</select>
						</div>
						<div>
							<Label>Quality Score ({minQualityScore}%)</Label>
							<input type="range" class="w-full" min="50" max="99" bind:value={minQualityScore} />
						</div>
						<div>
							<Label>Timeout ({timeoutSeconds}s)</Label>
							<Input type="number" bind:value={timeoutSeconds} />
						</div>
					</div>
				{/if}

				<div class="mt-4 flex justify-end">
					<Button
						variant="ghost"
						size="sm"
						onclick={() => (showAdvanced = !showAdvanced)}
						class="text-muted-foreground text-xs"
					>
						{#if showAdvanced}ซ่อนค่าเทคนิค{:else}แสดงค่าเทคนิค (Advanced){/if}
					</Button>
				</div>
			</Card>

			<!-- Submit -->
			<div class="flex justify-end gap-3">
				<Button variant="outline" onclick={() => goto('/staff/academic/timetable')}>ยกเลิก</Button>
				<Button
					onclick={handleSubmit}
					disabled={submitting || selectedCount === 0}
					class="min-w-[150px]"
				>
					{#if submitting}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
						กำลังประมวลผล...
					{:else}
						<Zap class="mr-2 h-4 w-4" />
						เริ่มจัดตาราง
					{/if}
				</Button>
			</div>
		</div>
	{/if}
</div>
