<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getApplication,
		updateApplicationStatus,
		getApplicationLogs,
		createInterview,
		updateInterview,
		APPLICATION_STATUS_LABELS,
		APPLICATION_STATUS_COLORS,
		type AdmissionApplication,
		type AdmissionInterview,
		type ApplicationStatus
	} from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
	import XCircle from 'lucide-svelte/icons/x-circle';
	import CalendarPlus from 'lucide-svelte/icons/calendar-plus';
	import FileText from 'lucide-svelte/icons/file-text';
	import User from 'lucide-svelte/icons/user';
	import History from 'lucide-svelte/icons/history';
	import ClipboardList from 'lucide-svelte/icons/clipboard-list';

	let { data } = $props();
	const { periodId, appId } = data;

	let app = $state<AdmissionApplication | null>(null);
	let interviews = $state<AdmissionInterview[]>([]);
	let documents = $state<any[]>([]);
	let logs = $state<any[]>([]);
	let loading = $state(true);

	// Status change
	let showStatusDialog = $state(false);
	let newStatus = $state<ApplicationStatus>('reviewing');
	let staffNotes = $state('');
	let rejectionReason = $state('');
	let interviewScore = $state('');
	let examScore = $state('');
	let totalScore = $state('');
	let updatingStatus = $state(false);

	// Interview dialog
	let showInterviewDialog = $state(false);
	let interviewDate = $state('');
	let interviewTime = $state('');
	let interviewLocation = $state('');
	let creatingInterview = $state(false);

	async function loadAll() {
		try {
			loading = true;
			const [appRes, logsRes] = await Promise.all([
				getApplication(appId),
				getApplicationLogs(appId)
			]);
			app = appRes.data;
			interviews = appRes.interviews;
			documents = appRes.documents;
			logs = logsRes.data;
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleUpdateStatus() {
		if (!app) return;
		updatingStatus = true;
		try {
			await updateApplicationStatus(appId, {
				status: newStatus,
				staff_notes: staffNotes || undefined,
				rejection_reason: rejectionReason || undefined,
				interview_score: interviewScore ? parseFloat(interviewScore) : undefined,
				exam_score: examScore ? parseFloat(examScore) : undefined,
				total_score: totalScore ? parseFloat(totalScore) : undefined
			});
			toast.success(`อัปเดตสถานะเป็น "${APPLICATION_STATUS_LABELS[newStatus]}" เรียบร้อย`);
			showStatusDialog = false;
			await loadAll();
		} catch (e: any) {
			toast.error(e.message || 'อัปเดตไม่สำเร็จ');
		} finally {
			updatingStatus = false;
		}
	}

	async function handleCreateInterview() {
		if (!interviewDate) {
			toast.error('กรุณาเลือกวันที่');
			return;
		}
		creatingInterview = true;
		try {
			const scheduledAt = interviewTime
				? `${interviewDate}T${interviewTime}:00`
				: `${interviewDate}T09:00:00`;
			await createInterview({
				application_id: appId,
				interview_type: 'interview',
				scheduled_at: scheduledAt,
				location: interviewLocation || undefined
			});
			toast.success('บันทึกนัดสัมภาษณ์เรียบร้อยแล้ว');
			showInterviewDialog = false;
			interviewDate = '';
			interviewTime = '';
			interviewLocation = '';
			await loadAll();
		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			creatingInterview = false;
		}
	}

	function openStatusDialog(status: ApplicationStatus) {
		newStatus = status;
		staffNotes = '';
		rejectionReason = '';
		interviewScore = '';
		examScore = '';
		totalScore = '';
		showStatusDialog = true;
	}

	function formatDate(d: string) {
		return new Date(d).toLocaleDateString('th-TH', {
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
	}

	function formatDateTime(d: string) {
		return new Date(d).toLocaleString('th-TH', {
			day: 'numeric',
			month: 'short',
			year: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	onMount(loadAll);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="mx-auto max-w-5xl space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" href="/staff/academic/admission/{periodId}">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div class="flex-1">
			{#if app}
				<div class="flex flex-wrap items-center gap-2">
					<h1 class="text-xl font-bold">
						{app.applicant_title || ''}{app.applicant_first_name}
						{app.applicant_last_name}
					</h1>
					<span
						class="rounded-full border px-2.5 py-0.5 text-xs font-medium {APPLICATION_STATUS_COLORS[
							app.status
						]}"
					>
						{APPLICATION_STATUS_LABELS[app.status]}
					</span>
				</div>
				<p class="text-sm text-muted-foreground font-mono">เลขที่ {app.application_number}</p>
			{:else if loading}
				<div class="h-6 w-48 animate-pulse rounded bg-muted"></div>
			{/if}
		</div>
	</div>

	{#if loading && !app}
		<div class="flex h-64 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else if app}
		<!-- Quick Actions -->
		<Card.Root class="border-primary/20 bg-primary/5">
			<Card.Content class="pt-4 pb-4">
				<div class="flex flex-wrap items-center gap-2">
					<span class="text-sm font-medium mr-2">เปลี่ยนสถานะ:</span>

					{#if app.status === 'pending'}
						<Button size="sm" onclick={() => openStatusDialog('reviewing')} variant="outline">
							เริ่มพิจารณา
						</Button>
					{/if}

					{#if ['pending', 'reviewing', 'interview_scheduled'].includes(app.status)}
						<Button
							size="sm"
							class="border-green-400 bg-green-600 text-white hover:bg-green-700 gap-1.5"
							onclick={() => openStatusDialog('accepted')}
						>
							<CheckCircle2 class="h-3.5 w-3.5" /> อนุมัติผ่าน
						</Button>
						<Button
							size="sm"
							variant="outline"
							class="border-orange-400 text-orange-700 hover:bg-orange-50 gap-1.5"
							onclick={() => openStatusDialog('waitlisted')}
						>
							รายชื่อสำรอง
						</Button>
						<Button
							size="sm"
							variant="outline"
							class="border-red-400 text-red-700 hover:bg-red-50 gap-1.5"
							onclick={() => openStatusDialog('rejected')}
						>
							<XCircle class="h-3.5 w-3.5" /> ไม่ผ่าน
						</Button>
					{/if}

					<Button
						size="sm"
						variant="outline"
						class="ml-auto gap-1.5"
						onclick={() => (showInterviewDialog = true)}
					>
						<CalendarPlus class="h-3.5 w-3.5" /> นัดสัมภาษณ์
					</Button>
				</div>
			</Card.Content>
		</Card.Root>

		<div class="grid gap-6 lg:grid-cols-3">
			<!-- Left: Application Details -->
			<div class="space-y-6 lg:col-span-2">
				<!-- ข้อมูลผู้สมัคร -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<User class="h-5 w-5" />
							ข้อมูลผู้สมัคร
						</Card.Title>
					</Card.Header>
					<Card.Content>
						<dl class="grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
							<div>
								<dt class="text-muted-foreground mb-0.5">ชื่อ-นามสกุล</dt>
								<dd class="font-medium">
									{app.applicant_title || ''}{app.applicant_first_name}
									{app.applicant_last_name}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">เพศ</dt>
								<dd>
									{app.applicant_gender === 'male'
										? 'ชาย'
										: app.applicant_gender === 'female'
											? 'หญิง'
											: '-'}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">เลขบัตรประชาชน</dt>
								<dd class="font-mono">{app.applicant_national_id || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">วันเกิด</dt>
								<dd>
									{app.applicant_date_of_birth ? formatDate(app.applicant_date_of_birth) : '-'}
								</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">หมู่โลหิต</dt>
								<dd>{app.applicant_blood_type || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">สัญชาติ</dt>
								<dd>{app.applicant_nationality || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">ศาสนา</dt>
								<dd>{app.applicant_religion || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">เบอร์โทร</dt>
								<dd>{app.applicant_phone || '-'}</dd>
							</div>
							<div class="col-span-2">
								<dt class="text-muted-foreground mb-0.5">ที่อยู่</dt>
								<dd>{app.applicant_address || '-'}</dd>
							</div>
						</dl>
					</Card.Content>
				</Card.Root>

				<!-- โรงเรียนเดิม -->
				<Card.Root>
					<Card.Header>
						<Card.Title>ข้อมูลการศึกษาเดิม</Card.Title>
					</Card.Header>
					<Card.Content>
						<dl class="grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
							<div>
								<dt class="text-muted-foreground mb-0.5">โรงเรียนเดิม</dt>
								<dd>{app.previous_school || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">ชั้นที่กำลังเรียน/จบ</dt>
								<dd>{app.previous_grade || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">ผลการเรียน GPA</dt>
								<dd class="font-mono">{app.previous_gpa?.toFixed(2) || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">สมัครเข้าระดับชั้น</dt>
								<dd>{app.grade_level_name || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">ความต้องการพิเศษ</dt>
								<dd>{app.applying_classroom_preference || '-'}</dd>
							</div>
						</dl>
					</Card.Content>
				</Card.Root>

				<!-- ผู้ปกครอง -->
				<Card.Root>
					<Card.Header>
						<Card.Title>ข้อมูลผู้ปกครอง</Card.Title>
					</Card.Header>
					<Card.Content>
						<dl class="grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
							<div>
								<dt class="text-muted-foreground mb-0.5">ชื่อผู้ปกครอง</dt>
								<dd class="font-medium">{app.guardian_name || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">ความสัมพันธ์</dt>
								<dd>{app.guardian_relationship || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">เบอร์โทร</dt>
								<dd>{app.guardian_phone || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">อีเมล</dt>
								<dd>{app.guardian_email || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">อาชีพ</dt>
								<dd>{app.guardian_occupation || '-'}</dd>
							</div>
							<div>
								<dt class="text-muted-foreground mb-0.5">เลขบัตรปชช. ผู้ปกครอง</dt>
								<dd class="font-mono">{app.guardian_national_id || '-'}</dd>
							</div>
						</dl>
					</Card.Content>
				</Card.Root>

				<!-- Scores & Notes -->
				{#if app.status !== 'pending' && app.status !== 'reviewing'}
					<Card.Root>
						<Card.Header>
							<Card.Title>ผลคะแนนและหมายเหตุ</Card.Title>
						</Card.Header>
						<Card.Content>
							<dl class="grid grid-cols-3 gap-4 text-sm mb-4">
								<div class="text-center rounded-lg bg-muted/50 p-3">
									<dt class="text-muted-foreground text-xs mb-1">คะแนนสัมภาษณ์</dt>
									<dd class="text-xl font-bold">{app.interview_score?.toFixed(1) || '-'}</dd>
								</div>
								<div class="text-center rounded-lg bg-muted/50 p-3">
									<dt class="text-muted-foreground text-xs mb-1">คะแนนสอบ</dt>
									<dd class="text-xl font-bold">{app.exam_score?.toFixed(1) || '-'}</dd>
								</div>
								<div class="text-center rounded-lg bg-primary/10 p-3">
									<dt class="text-muted-foreground text-xs mb-1">คะแนนรวม</dt>
									<dd class="text-xl font-bold text-primary">
										{app.total_score?.toFixed(1) || '-'}
									</dd>
								</div>
							</dl>
							{#if app.staff_notes}
								<div>
									<p class="text-xs text-muted-foreground mb-1">หมายเหตุ</p>
									<p class="text-sm bg-muted/50 rounded-lg px-3 py-2">{app.staff_notes}</p>
								</div>
							{/if}
							{#if app.rejection_reason}
								<div class="mt-3">
									<p class="text-xs text-red-500 mb-1">เหตุผลที่ปฏิเสธ</p>
									<p
										class="text-sm bg-red-50 text-red-800 rounded-lg px-3 py-2 dark:bg-red-950/20 dark:text-red-300"
									>
										{app.rejection_reason}
									</p>
								</div>
							{/if}
						</Card.Content>
					</Card.Root>
				{/if}

				<!-- Interviews -->
				{#if interviews.length > 0}
					<Card.Root>
						<Card.Header>
							<Card.Title>ประวัติการสัมภาษณ์</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-3">
							{#each interviews as iv}
								<div class="flex items-start gap-3 rounded-lg border p-3">
									<CalendarPlus class="h-5 w-5 text-primary mt-0.5 shrink-0" />
									<div class="flex-1 text-sm">
										<p class="font-medium">
											{iv.interview_type === 'interview'
												? 'สัมภาษณ์'
												: iv.interview_type === 'exam'
													? 'สอบข้อเขียน'
													: 'ประเมิน'}
											<span
												class="ml-2 text-xs rounded-full px-2 py-0.5 {iv.status === 'completed'
													? 'bg-green-100 text-green-700'
													: 'bg-yellow-100 text-yellow-700'}"
											>
												{iv.status === 'completed'
													? 'เสร็จแล้ว'
													: iv.status === 'scheduled'
														? 'นัดแล้ว'
														: iv.status}
											</span>
										</p>
										{#if iv.scheduled_at}
											<p class="text-muted-foreground mt-0.5">{formatDateTime(iv.scheduled_at)}</p>
										{/if}
										{#if iv.location}
											<p class="text-muted-foreground">{iv.location}</p>
										{/if}
										{#if iv.score != null}
											<p class="mt-1 font-semibold">คะแนน: {iv.score} / {iv.max_score || 100}</p>
										{/if}
									</div>
								</div>
							{/each}
						</Card.Content>
					</Card.Root>
				{/if}
			</div>

			<!-- Right: Audit Log + Documents -->
			<div class="space-y-6">
				<!-- Documents -->
				{#if documents.length > 0}
					<Card.Root>
						<Card.Header>
							<Card.Title class="flex items-center gap-2">
								<FileText class="h-5 w-5" />
								เอกสารแนบ
							</Card.Title>
						</Card.Header>
						<Card.Content class="space-y-2">
							{#each documents as doc}
								<a
									href={doc.file_url}
									target="_blank"
									rel="noopener noreferrer"
									class="flex items-center gap-2 rounded-lg border p-3 hover:bg-muted/50 transition-colors text-sm group"
								>
									<FileText class="h-4 w-4 text-muted-foreground" />
									<span class="flex-1 truncate">{doc.document_label || doc.document_key}</span>
									<span class="text-xs text-muted-foreground group-hover:text-primary">ดูไฟล์</span>
								</a>
							{/each}
						</Card.Content>
					</Card.Root>
				{/if}

				<!-- Audit Log -->
				<Card.Root>
					<Card.Header>
						<Card.Title class="flex items-center gap-2">
							<History class="h-5 w-5" />
							ประวัติการดำเนินการ
						</Card.Title>
					</Card.Header>
					<Card.Content>
						{#if logs.length === 0}
							<p class="text-sm text-muted-foreground text-center py-4">ยังไม่มีประวัติ</p>
						{:else}
							<div class="relative space-y-3">
								{#each logs as log}
									<div class="relative pl-4 border-l-2 border-border text-sm">
										<div class="absolute -left-[5px] top-1 h-2 w-2 rounded-full bg-primary"></div>
										<p class="font-medium">
											{log.action === 'status_changed'
												? `เปลี่ยนสถานะ: ${log.old_value} → ${log.new_value}`
												: log.action}
										</p>
										{#if log.note}
											<p class="text-muted-foreground text-xs mt-0.5">{log.note}</p>
										{/if}
										<p class="text-xs text-muted-foreground mt-0.5">
											{log.performer_name || 'ระบบ'} · {formatDateTime(log.performed_at)}
										</p>
									</div>
								{/each}
							</div>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</div>

<!-- Status Update Dialog -->
<Dialog.Root bind:open={showStatusDialog}>
	<Dialog.Content class="sm:max-w-[480px]">
		<Dialog.Header>
			<Dialog.Title>เปลี่ยนสถานะใบสมัคร</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<Label>สถานะใหม่</Label>
				<Select.Root type="single" bind:value={newStatus}>
					<Select.Trigger class="w-full">{APPLICATION_STATUS_LABELS[newStatus]}</Select.Trigger>
					<Select.Content>
						{#each Object.entries(APPLICATION_STATUS_LABELS) as [val, label]}
							<Select.Item value={val}>{label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			{#if newStatus === 'rejected'}
				<div class="space-y-2">
					<Label>เหตุผลที่ปฏิเสธ <span class="text-red-500">*</span></Label>
					<Textarea bind:value={rejectionReason} placeholder="กรุณาระบุเหตุผล..." rows={3} />
				</div>
			{/if}

			{#if ['accepted', 'waitlisted'].includes(newStatus)}
				<div class="grid grid-cols-3 gap-3">
					<div class="space-y-2">
						<Label>คะแนนสัมภาษณ์</Label>
						<Input type="number" bind:value={interviewScore} placeholder="0" min="0" max="100" />
					</div>
					<div class="space-y-2">
						<Label>คะแนนสอบ</Label>
						<Input type="number" bind:value={examScore} placeholder="0" min="0" max="100" />
					</div>
					<div class="space-y-2">
						<Label>คะแนนรวม</Label>
						<Input type="number" bind:value={totalScore} placeholder="0" min="0" max="100" />
					</div>
				</div>
			{/if}

			<div class="space-y-2">
				<Label>หมายเหตุ (ไม่บังคับ)</Label>
				<Textarea bind:value={staffNotes} placeholder="บันทึกเพิ่มเติม..." rows={2} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showStatusDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleUpdateStatus} disabled={updatingStatus}>
				{#if updatingStatus}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				ยืนยัน
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Interview Dialog -->
<Dialog.Root bind:open={showInterviewDialog}>
	<Dialog.Content class="sm:max-w-[440px]">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<CalendarPlus class="h-5 w-5 text-primary" />
				นัดสัมภาษณ์
			</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-4 py-4">
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-2">
					<Label for="ivDate">วันที่ <span class="text-red-500">*</span></Label>
					<Input id="ivDate" type="date" bind:value={interviewDate} />
				</div>
				<div class="space-y-2">
					<Label for="ivTime">เวลา</Label>
					<Input id="ivTime" type="time" bind:value={interviewTime} />
				</div>
			</div>
			<div class="space-y-2">
				<Label for="ivLoc">สถานที่</Label>
				<Input
					id="ivLoc"
					bind:value={interviewLocation}
					placeholder="เช่น ห้องประชุม อาคาร 1 ชั้น 2"
				/>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showInterviewDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleCreateInterview} disabled={creatingInterview}>
				{#if creatingInterview}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				บันทึกนัด
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
