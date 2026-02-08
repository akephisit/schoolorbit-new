<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Progress } from '$lib/components/ui/progress';
	import {
		Loader2,
		CheckCircle2,
		XCircle,
		Clock,
		Zap,
		AlertCircle,
		ArrowLeft,
		Eye
	} from 'lucide-svelte';
	import type { SchedulingJobResponse } from '$lib/api/scheduling';
	import { getSchedulingJob } from '$lib/api/scheduling';

	const jobId = $page.params.jobId;

	let job: SchedulingJobResponse | null = null;
	let loading = true;
	let polling: ReturnType<typeof setInterval> | null = null;

	onMount(async () => {
		await loadJob();
		startPolling();
	});

	onDestroy(() => {
		stopPolling();
	});

	async function loadJob() {
		if (!jobId) {
			loading = false;
			return;
		}

		try {
			const res = await getSchedulingJob(jobId as string);
			job = res.data || null;
			loading = false;

			// Stop polling if job is complete
			if (
				job &&
				(job.status === 'COMPLETED' || job.status === 'FAILED' || job.status === 'CANCELLED')
			) {
				stopPolling();
			}
		} catch (error) {
			console.error('Failed to load job:', error);
			toast.error('ไม่สามารถโหลดข้อมูลงานได้');
			loading = false;
		}
	}

	function startPolling() {
		// Poll every 2 seconds
		polling = setInterval(async () => {
			await loadJob();
		}, 2000);
	}

	function stopPolling() {
		if (polling) {
			clearInterval(polling);
			polling = null;
		}
	}

	function getStatusBadge(status: string) {
		switch (status) {
			case 'PENDING':
				return { variant: 'secondary' as const, icon: Clock, text: 'รอดำเนินการ' };
			case 'RUNNING':
				return { variant: 'default' as const, icon: Loader2, text: 'กำลังดำเนินการ' };
			case 'COMPLETED':
				return { variant: 'default' as const, icon: CheckCircle2, text: 'สำเร็จ' };
			case 'FAILED':
				return { variant: 'destructive' as const, icon: XCircle, text: 'ล้มเหลว' };
			case 'CANCELLED':
				return { variant: 'secondary' as const, icon: AlertCircle, text: 'ยกเลิก' };
			default:
				return { variant: 'secondary' as const, icon: Clock, text: status };
		}
	}

	function getQualityColor(score: number) {
		if (score >= 90) return 'text-green-600';
		if (score >= 80) return 'text-blue-600';
		if (score >= 70) return 'text-yellow-600';
		return 'text-orange-600';
	}

	$: statusBadge = job ? getStatusBadge(job.status) : null;
	$: isRunning = job?.status === 'RUNNING' || job?.status === 'PENDING';
</script>

{#if loading}
	<div class="flex justify-center py-12">
		<Loader2 class="h-8 w-8 animate-spin text-primary" />
	</div>
{:else if job}
	<div class="container mx-auto p-6 max-w-4xl">
		<!-- Header -->
		<div class="mb-6">
			<Button
				variant="ghost"
				size="sm"
				onclick={() => goto('/staff/academic/timetable/scheduling')}
			>
				<ArrowLeft class="mr-2 h-4 w-4" />
				กลับ
			</Button>

			<div class="mt-4">
				<div class="flex items-center justify-between mb-2">
					<h1 class="text-3xl font-bold">สถานะการจัดตาราง</h1>
					{#if statusBadge}
						<Badge variant={statusBadge.variant} class="px-3 py-1">
							<svelte:component this={statusBadge.icon} class="mr-1 h-4 w-4" />
							{statusBadge.text}
						</Badge>
					{/if}
				</div>
				<p class="text-muted-foreground">Job ID: {job.id}</p>
			</div>
		</div>

		<div class="space-y-6">
			<!-- Progress -->
			{#if isRunning}
				<Card class="p-6">
					<div class="mb-4">
						<div class="flex justify-between mb-2">
							<span class="font-medium">ความคืบหน้า</span>
							<span class="text-sm text-muted-foreground">{job.progress}%</span>
						</div>
						<Progress value={job.progress} max={100} class="h-3" />
					</div>

					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						{#if job.status === 'RUNNING'}
							<Loader2 class="h-4 w-4 animate-spin" />
							<span>กำลังประมวลผล...</span>
						{:else}
							<Clock class="h-4 w-4" />
							<span>รอดำเนินการ...</span>
						{/if}
					</div>
				</Card>
			{/if}

			<!-- Results -->
			{#if job.status === 'COMPLETED'}
				<Card class="p-6">
					<h2 class="text-xl font-semibold mb-4 flex items-center gap-2">
						<CheckCircle2 class="h-5 w-5 text-green-600" />
						ผลลัพธ์
					</h2>

					<div class="grid grid-cols-1 md:grid-cols-3 gap-4">
						<!-- Quality Score -->
						<div class="text-center p-4 rounded-lg bg-muted/50">
							<div class="text-sm text-muted-foreground mb-1">คะแนนคุณภาพ</div>
							<div class={`text-3xl font-bold ${getQualityColor(job.quality_score || 0)}`}>
								{job.quality_score?.toFixed(1)}%
							</div>
						</div>

						<!-- Scheduled Courses -->
						<div class="text-center p-4 rounded-lg bg-muted/50">
							<div class="text-sm text-muted-foreground mb-1">จัดสำเร็จ</div>
							<div class="text-3xl font-bold text-green-600">
								{job.scheduled_courses}
							</div>
							<div class="text-xs text-muted-foreground">
								จาก {job.total_courses} รายวิชา
							</div>
						</div>

						<!-- Duration -->
						<div class="text-center p-4 rounded-lg bg-muted/50">
							<div class="text-sm text-muted-foreground mb-1">ระยะเวลา</div>
							<div class="text-3xl font-bold">
								{job.duration_seconds || 0}
							</div>
							<div class="text-xs text-muted-foreground">วินาที</div>
						</div>
					</div>

					<!-- Failed Courses -->
					{#if job.failed_courses && job.failed_courses.length > 0}
						<div class="mt-6">
							<h3 class="font-semibold mb-3 flex items-center gap-2">
								<AlertCircle class="h-4 w-4 text-orange-600" />
								รายวิชาที่จัดไม่สำเร็จ ({job.failed_courses.length})
							</h3>

							<div class="space-y-2 max-h-[300px] overflow-y-auto">
								{#each job.failed_courses as failed}
									<div class="p-3 rounded-lg border bg-orange-50 dark:bg-orange-950/20">
										<div class="font-medium">{failed.subject_name || failed.subject_code}</div>
										<div class="text-sm text-muted-foreground">ห้อง: {failed.classroom}</div>
										<div class="text-sm text-orange-600">{failed.reason}</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</Card>
			{/if}

			<!-- Error Message -->
			{#if job.status === 'FAILED' && job.error_message}
				<Card class="p-6 border-destructive">
					<h2 class="text-xl font-semibold mb-4 flex items-center gap-2 text-destructive">
						<XCircle class="h-5 w-5" />
						ข้อผิดพลาด
					</h2>
					<p class="text-sm text-muted-foreground">{job.error_message}</p>
				</Card>
			{/if}

			<!-- Metadata -->
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-4">รายละเอียด</h2>

				<div class="space-y-2 text-sm">
					<div class="flex justify-between">
						<span class="text-muted-foreground">อัลกอริทึม:</span>
						<span class="font-medium">{job.algorithm}</span>
					</div>
					<div class="flex justify-between">
						<span class="text-muted-foreground">จำนวนห้องเรียน:</span>
						<span class="font-medium">{job.classroom_ids.length} ห้อง</span>
					</div>
					<div class="flex justify-between">
						<span class="text-muted-foreground">สร้างเมื่อ:</span>
						<span class="font-medium">
							{new Date(job.created_at).toLocaleString('th-TH')}
						</span>
					</div>
					{#if job.started_at}
						<div class="flex justify-between">
							<span class="text-muted-foreground">เริ่มดำเนินการ:</span>
							<span class="font-medium">
								{new Date(job.started_at).toLocaleString('th-TH')}
							</span>
						</div>
					{/if}
					{#if job.completed_at}
						<div class="flex justify-between">
							<span class="text-muted-foreground">เสร็จสิ้น:</span>
							<span class="font-medium">
								{new Date(job.completed_at).toLocaleString('th-TH')}
							</span>
						</div>
					{/if}
				</div>
			</Card>

			<!-- Actions -->
			{#if job.status === 'COMPLETED'}
				<div class="flex justify-end gap-3">
					<Button onclick={() => goto('/staff/academic/timetable')}>
						<Eye class="mr-2 h-4 w-4" />
						ดูตารางสอน
					</Button>
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div class="container mx-auto p-6 max-w-4xl">
		<Card class="p-12 text-center">
			<AlertCircle class="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
			<h2 class="text-xl font-semibold mb-2">ไม่พบงาน</h2>
			<p class="text-muted-foreground mb-4">ไม่พบงานที่คุณต้องการ</p>
			<Button onclick={() => goto('/staff/academic/timetable/scheduling')}>กลับ</Button>
		</Card>
	</div>
{/if}
