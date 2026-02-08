<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import {
		CalendarDays,
		Clock,
		CheckCircle2,
		XCircle,
		Loader2,
		AlertCircle,
		ArrowLeft,
		Plus,
		History
	} from 'lucide-svelte';
	import * as Select from '$lib/components/ui/select';
	import { listSchedulingJobs, type SchedulingJobResponse } from '$lib/api/scheduling';
	import { getAcademicStructure } from '$lib/api/academic';

	// State
	let jobs = $state<SchedulingJobResponse[]>([]);
	let loading = $state(true);
	let semesterId = $state<string>('');
	let allSemesters = $state<any[]>([]);

	// Selected semester object for Select component
	let selectedSemester = $derived(
		allSemesters.find((s) => s.id === semesterId)
			? {
					value: semesterId,
					label: `${allSemesters.find((s) => s.id === semesterId)?.term}/${
						allSemesters.find((s) => s.id === semesterId)?.academic_year_code
					}`
				}
			: undefined
	);

	onMount(async () => {
		await loadSemesters();
		// Load jobs will be triggered by $effect when semesterId changes
	});

	async function loadSemesters() {
		try {
			const res = await getAcademicStructure();
			allSemesters = res.data.semesters;

			// Auto-select active semester if available
			const active = allSemesters.find((s) => s.is_active);
			if (active) semesterId = active.id;
			else if (allSemesters.length > 0) semesterId = allSemesters[0].id;
		} catch (error) {
			console.error('Failed to load semesters:', error);
		}
	}

	async function loadJobs() {
		if (!semesterId) return;

		loading = true;
		try {
			const res = await listSchedulingJobs({ semester_id: semesterId });
			jobs = res.data || [];
		} catch (error) {
			console.error('Failed to load jobs:', error);
			jobs = [];
		} finally {
			loading = false;
		}
	}

	function getStatusBadge(status: string) {
		switch (status) {
			case 'PENDING':
				return {
					variant: 'secondary' as const,
					icon: Clock,
					text: 'รอดำเนินการ',
					color: 'text-gray-500'
				};
			case 'RUNNING':
				return {
					variant: 'default' as const,
					icon: Loader2,
					text: 'กำลังดำเนินการ',
					color: 'text-blue-500'
				};
			case 'COMPLETED':
				return {
					variant: 'default' as const,
					icon: CheckCircle2,
					text: 'สำเร็จ',
					color: 'text-green-500'
				};
			case 'FAILED':
				return {
					variant: 'destructive' as const,
					icon: XCircle,
					text: 'ล้มเหลว',
					color: 'text-red-500'
				};
			case 'CANCELLED':
				return {
					variant: 'secondary' as const,
					icon: AlertCircle,
					text: 'ยกเลิก',
					color: 'text-gray-500'
				};
			default:
				return { variant: 'secondary' as const, icon: Clock, text: status, color: 'text-gray-500' };
		}
	}

	function formatDate(dateStr: string) {
		return new Date(dateStr).toLocaleString('th-TH');
	}

	$effect(() => {
		if (semesterId) {
			loadJobs();
		}
	});
</script>

<div class="container mx-auto p-6 max-w-6xl">
	<!-- Header -->
	<div class="flex items-center justify-between mb-6">
		<div class="flex items-center gap-4">
			<Button
				variant="ghost"
				size="icon"
				onclick={() => goto('/staff/academic/timetable/scheduling/auto-schedule')}
			>
				<ArrowLeft class="h-4 w-4" />
			</Button>
			<div>
				<h1 class="text-3xl font-bold flex items-center gap-2">
					<History class="h-8 w-8" />
					ประวัติการจัดตาราง
				</h1>
				<p class="text-muted-foreground">รายการจัดตารางทั้งหมดในระบบ</p>
			</div>
		</div>
		<Button onclick={() => goto('/staff/academic/timetable/scheduling/auto-schedule')}>
			<Plus class="mr-2 h-4 w-4" />
			สร้างรายการใหม่
		</Button>
	</div>

	<!-- Filters -->
	<div class="mb-6 flex gap-4">
		<div class="w-[300px]">
			<Select.Root
				type="single"
				value={semesterId}
				onValueChange={(v) => {
					if (v) semesterId = v;
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

	<!-- Table -->
	<Card class="p-0 overflow-hidden">
		{#if loading}
			<div class="p-12 flex justify-center">
				<Loader2 class="h-8 w-8 animate-spin text-primary" />
			</div>
		{:else if jobs.length === 0}
			<div class="p-12 text-center text-muted-foreground">
				<History class="h-12 w-12 mx-auto mb-4 opacity-50" />
				<p>ไม่พบประวัติการจัดตารางในภาคเรียนนี้</p>
			</div>
		{:else}
			<Table>
				<TableHeader>
					<TableRow>
						<TableHead>สถานะ</TableHead>
						<TableHead>วันที่สร้าง</TableHead>
						<TableHead>อัลกอริทึม</TableHead>
						<TableHead class="text-center">ความคืบหน้า</TableHead>
						<TableHead class="text-center">คะแนนคุณภาพ</TableHead>
						<TableHead class="text-center">วิชาที่สำเร็จ</TableHead>
						<TableHead class="text-right">Action</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{#each jobs as job}
						{@const status = getStatusBadge(job.status)}
						<TableRow
							class="hover:bg-muted/50 cursor-pointer"
							onclick={() => goto(`/staff/academic/timetable/scheduling/jobs/${job.id}`)}
						>
							<TableCell>
								<div class="flex items-center gap-2">
									<status.icon class={`h-4 w-4 ${status.color}`} />
									<span class={status.color}>{status.text}</span>
								</div>
							</TableCell>
							<TableCell>{formatDate(job.created_at)}</TableCell>
							<TableCell>
								<Badge variant="outline">{job.algorithm}</Badge>
							</TableCell>
							<TableCell class="text-center">
								<div class="flex items-center justify-center gap-2">
									<div class="w-16 h-1.5 bg-secondary rounded-full overflow-hidden">
										<div
											class="h-full bg-primary transition-all"
											style="width: {job.progress}%"
										></div>
									</div>
									<span class="text-xs text-muted-foreground">{job.progress}%</span>
								</div>
							</TableCell>
							<TableCell class="text-center font-medium">
								{job.quality_score ? `${job.quality_score.toFixed(1)}%` : '-'}
							</TableCell>
							<TableCell class="text-center">
								{job.scheduled_courses} / {job.total_courses}
							</TableCell>
							<TableCell class="text-right">
								<Button variant="ghost" size="sm">ดูรายละเอียด</Button>
							</TableCell>
						</TableRow>
					{/each}
				</TableBody>
			</Table>
		{/if}
	</Card>
</div>
