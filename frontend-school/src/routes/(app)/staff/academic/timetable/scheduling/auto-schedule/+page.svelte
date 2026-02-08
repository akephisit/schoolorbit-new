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
	let selectedSemesterId = $state<UUID | null>(null);
	let selectedClassroomIds = $state<UUID[]>([]);

	let selectedSemester = $derived(
		allSemesters.find((s) => s.id === selectedSemesterId)
			? {
					value: selectedSemesterId,
					label: `${allSemesters.find((s) => s.id === selectedSemesterId)?.term}/${
						allSemesters.find((s) => s.id === selectedSemesterId)?.academic_year_code
					}`
				}
			: undefined
	);

	// Config
	let algorithm = $state<SchedulingAlgorithm>('BACKTRACKING');
	let forceOverwrite = $state(false);
	let allowPartial = $state(false);
	let minQualityScore = $state(70);
	let timeoutSeconds = $state(120);

	onMount(async () => {
		await loadSemesters();
		await loadClassrooms();

		// Get semester from query params if available
		const semesterId = $page.url.searchParams.get('semester_id');
		if (semesterId) {
			selectedSemesterId = semesterId;
		}
	});

	async function loadSemesters() {
		try {
			const res = await getAcademicStructure();
			allSemesters = res.data.semesters;

			// Auto-select active semester
			if (!selectedSemesterId) {
				const active = allSemesters.find((s) => s.is_active);
				if (active) selectedSemesterId = active.id;
				else if (allSemesters.length > 0) selectedSemesterId = allSemesters[0].id;
			}
		} catch (error) {
			console.error('Failed to load semesters:', error);
		}
	}

	async function loadClassrooms() {
		loading = true;
		try {
			const res = await listClassrooms();
			classrooms = res.data || [];
		} catch (error) {
			toast.error('โหลดข้อมูลห้องเรียนไม่สำเร็จ');
		} finally {
			loading = false;
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
					timeout_seconds: timeoutSeconds
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
		<Button variant="outline" onclick={() => goto('/staff/academic/timetable/scheduling/jobs')}>
			<History class="mr-2 h-4 w-4" />
			ประวัติการจัดตาราง
		</Button>
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
								<Select.Item
									value={semester.id}
									label={`${semester.term}/${semester.academic_year_code}`}
								>
									{semester.term}/{semester.academic_year_code}
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

			<!-- Algorithm Selection -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-4 flex items-center gap-2">
					<Settings2 class="h-5 w-5" />
					การตั้งค่า
				</h2>

				<div class="space-y-4">
					<!-- Algorithm -->
					<div>
						<Label for="algorithm">อัลกอริทึม</Label>
						<select
							id="algorithm"
							bind:value={algorithm}
							class="w-full mt-1 rounded-md border border-input bg-background px-3 py-2"
						>
							<option value="BACKTRACKING">Backtracking (คุณภาพสูง)</option>
							<option value="GREEDY">Greedy (รวดเร็ว)</option>
							<option value="HYBRID">Hybrid (สมดุล)</option>
						</select>
						<p class="text-xs text-muted-foreground mt-1">
							{#if algorithm === 'BACKTRACKING'}
								ใช้เวลานานกว่า แต่ได้ผลลัพธ์ที่ดีที่สุด
							{:else if algorithm === 'GREEDY'}
								รวดเร็ว เหมาะกับงานเร่งด่วน
							{:else}
								สมดุลระหว่างความเร็วและคุณภาพ
							{/if}
						</p>
					</div>

					<!-- Quality Score -->
					<div>
						<Label for="quality">คะแนนคุณภาพขั้นต่ำ: {minQualityScore}%</Label>
						<input
							id="quality"
							type="range"
							min="50"
							max="95"
							step="5"
							bind:value={minQualityScore}
							class="w-full"
						/>
					</div>

					<!-- Timeout -->
					<div>
						<Label for="timeout">เวลาสูงสุด (วินาที)</Label>
						<Input
							id="timeout"
							type="number"
							min="30"
							max="600"
							bind:value={timeoutSeconds}
							class="mt-1"
						/>
					</div>

					<!-- Options -->
					<div class="space-y-2 pt-2 border-t">
						<label class="flex items-center space-x-2">
							<Checkbox bind:checked={forceOverwrite} />
							<span class="text-sm">เขียนทับตารางเดิม</span>
						</label>

						<label class="flex items-center space-x-2">
							<Checkbox bind:checked={allowPartial} />
							<span class="text-sm">อนุญาตให้จัดไม่ครบ (บางวิชา)</span>
						</label>
					</div>
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
