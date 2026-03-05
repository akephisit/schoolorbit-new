<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getAdmissionPeriod,
		getAdmissionPeriodStats,
		listApplications,
		updateApplicationStatus,
		listSelections,
		createSelections,
		confirmSelection,
		generateStudents,
		APPLICATION_STATUS_LABELS,
		APPLICATION_STATUS_COLORS,
		PERIOD_STATUS_LABELS,
		PERIOD_STATUS_COLORS,
		type AdmissionPeriod,
		type AdmissionApplication,
		type AdmissionStats,
		type AdmissionSelection,
		type ApplicationStatus
	} from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Textarea } from '$lib/components/ui/textarea';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import Users from 'lucide-svelte/icons/users';
	import ClipboardList from 'lucide-svelte/icons/clipboard-list';
	import Search from 'lucide-svelte/icons/search';
	import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
	import XCircle from 'lucide-svelte/icons/x-circle';
	import Eye from 'lucide-svelte/icons/eye';
	import UserCheck from 'lucide-svelte/icons/user-check';
	import Sparkles from 'lucide-svelte/icons/sparkles';

	let { data } = $props();
	const periodId = data.periodId;

	let period = $state<AdmissionPeriod | null>(null);
	let stats = $state<AdmissionStats | null>(null);
	let applications = $state<AdmissionApplication[]>([]);
	let selections = $state<AdmissionSelection[]>([]);
	let loading = $state(true);
	let activeTab = $state('applications');

	// Filter state
	let statusFilter = $state('');
	let searchQuery = $state('');
	let currentPage = $state(1);
	let totalPages = $state(1);
	let totalApps = $state(0);

	// Bulk selection
	let selectedAppIds = $state<string[]>([]);
	let bulkStatus = $state<ApplicationStatus>('reviewing');
	let bulkNotes = $state('');
	let showBulkDialog = $state(false);
	let bulkSubmitting = $state(false);

	// Generate students dialog
	let showGenerateDialog = $state(false);
	let generating = $state(false);
	let generateResult = $state<{
		created_count: number;
		skipped_count: number;
		message: string;
	} | null>(null);

	async function loadAll() {
		try {
			loading = true;
			const [pRes, sRes] = await Promise.all([
				getAdmissionPeriod(periodId),
				getAdmissionPeriodStats(periodId)
			]);
			period = pRes.data;
			stats = sRes.data;
			await Promise.all([loadApplications(), loadSelections()]);
		} catch (e) {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function loadApplications() {
		try {
			const res = await listApplications({
				admission_period_id: periodId,
				status: statusFilter || undefined,
				search: searchQuery || undefined,
				page: currentPage,
				page_size: 20
			});
			applications = res.data;
			totalApps = res.total;
			totalPages = res.total_pages;
		} catch {
			toast.error('โหลดรายการใบสมัครไม่สำเร็จ');
		}
	}

	async function loadSelections() {
		try {
			const res = await listSelections(periodId);
			selections = res.data;
		} catch {
			toast.error('โหลดรายชื่อผู้ผ่านคัดเลือกไม่สำเร็จ');
		}
	}

	function toggleApp(id: string) {
		if (selectedAppIds.includes(id)) {
			selectedAppIds = selectedAppIds.filter((x) => x !== id);
		} else {
			selectedAppIds = [...selectedAppIds, id];
		}
	}

	function toggleAllApps() {
		if (selectedAppIds.length === applications.length) {
			selectedAppIds = [];
		} else {
			selectedAppIds = applications.map((a) => a.id);
		}
	}

	async function executeBulkStatus() {
		bulkSubmitting = true;
		let success = 0;
		for (const id of selectedAppIds) {
			try {
				await updateApplicationStatus(id, {
					status: bulkStatus,
					staff_notes: bulkNotes || undefined
				});
				success++;
			} catch {}
		}
		toast.success(`อัปเดตสถานะสำเร็จ ${success} ราย`);
		showBulkDialog = false;
		selectedAppIds = [];
		bulkNotes = '';
		await loadApplications();
		bulkSubmitting = false;
	}

	async function handleConfirmSelection(selId: string) {
		try {
			await confirmSelection(selId);
			toast.success('ยืนยันสิทธิ์เรียบร้อยแล้ว');
			await loadSelections();
		} catch (e: any) {
			toast.error(e.message || 'ยืนยันสิทธิ์ไม่สำเร็จ');
		}
	}

	async function handleGenerateStudents() {
		generating = true;
		generateResult = null;
		try {
			const res = await generateStudents(periodId, {});
			generateResult = res;
			toast.success(res.message);
			await loadSelections();
		} catch (e: any) {
			toast.error(e.message || 'สร้าง account ไม่สำเร็จ');
		} finally {
			generating = false;
		}
	}

	function formatDate(d: string) {
		return new Date(d).toLocaleDateString('th-TH', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
	}

	onMount(loadAll);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-start gap-4">
		<Button variant="ghost" size="icon" href="/staff/academic/admission" class="mt-0.5 shrink-0">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div class="flex-1 min-w-0">
			{#if period}
				<div class="flex flex-wrap items-center gap-2">
					<h1 class="text-2xl font-bold text-foreground">{period.name}</h1>
					<span
						class="rounded-full px-2.5 py-0.5 text-xs font-medium {PERIOD_STATUS_COLORS[
							period.status
						]}"
					>
						{PERIOD_STATUS_LABELS[period.status]}
					</span>
				</div>
				<p class="mt-1 text-sm text-muted-foreground">
					{formatDate(period.open_date)} — {formatDate(period.close_date)}
					{#if period.academic_year_name}· ปีการศึกษา {period.academic_year_name}{/if}
				</p>
			{:else if loading}
				<div class="h-8 w-48 animate-pulse rounded bg-muted"></div>
			{/if}
		</div>
		{#if period}
			<Button href="/staff/academic/admission/{periodId}/edit" variant="outline" size="sm"
				>แก้ไขรอบ</Button
			>
		{/if}
	</div>

	{#if loading && !period}
		<div class="flex h-64 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else}
		<!-- Stats Cards -->
		{#if stats}
			<div class="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-8">
				{#each [{ label: 'ทั้งหมด', value: stats.total, color: 'text-foreground' }, { label: 'รอพิจารณา', value: stats.pending, color: 'text-yellow-600' }, { label: 'กำลังพิจารณา', value: stats.reviewing, color: 'text-blue-600' }, { label: 'นัดสัมภาษณ์', value: stats.reviewing, color: 'text-purple-600' }, { label: 'ผ่าน', value: stats.accepted, color: 'text-green-600' }, { label: 'ไม่ผ่าน', value: stats.rejected, color: 'text-red-600' }, { label: 'สำรอง', value: stats.waitlisted, color: 'text-orange-600' }, { label: 'ยืนยันแล้ว', value: stats.confirmed, color: 'text-emerald-600' }] as s}
					<Card.Root class="text-center">
						<Card.Content class="pt-4 pb-3">
							<p class="text-2xl font-bold {s.color}">{s.value}</p>
							<p class="text-xs text-muted-foreground">{s.label}</p>
						</Card.Content>
					</Card.Root>
				{/each}
			</div>
		{/if}

		<Tabs.Root bind:value={activeTab}>
			<Tabs.List class="grid w-full grid-cols-2 sm:w-auto sm:inline-flex">
				<Tabs.Trigger value="applications" class="flex items-center gap-2">
					<ClipboardList class="h-4 w-4" />
					ใบสมัคร ({totalApps})
				</Tabs.Trigger>
				<Tabs.Trigger value="selections" class="flex items-center gap-2">
					<UserCheck class="h-4 w-4" />
					ผู้ผ่านคัดเลือก ({selections.length})
				</Tabs.Trigger>
			</Tabs.List>

			<!-- ==================== TAB: Applications ==================== -->
			<Tabs.Content value="applications" class="space-y-4 mt-4">
				<!-- Filter bar -->
				<Card.Root>
					<Card.Content class="pt-4 pb-4">
						<div class="flex flex-col gap-3 sm:flex-row">
							<div class="relative flex-1">
								<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
								<Input
									placeholder="ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร..."
									class="pl-9"
									bind:value={searchQuery}
									oninput={() => {
										currentPage = 1;
										loadApplications();
									}}
								/>
							</div>
							<Select.Root
								type="single"
								bind:value={statusFilter}
								onValueChange={() => {
									currentPage = 1;
									loadApplications();
								}}
							>
								<Select.Trigger class="w-full sm:w-48">
									{statusFilter
										? APPLICATION_STATUS_LABELS[statusFilter as ApplicationStatus]
										: 'ทุกสถานะ'}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value="">ทุกสถานะ</Select.Item>
									{#each Object.entries(APPLICATION_STATUS_LABELS) as [val, label]}
										<Select.Item value={val}>{label}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					</Card.Content>
				</Card.Root>

				<!-- Bulk Actions -->
				{#if selectedAppIds.length > 0}
					<div class="flex items-center gap-3 rounded-lg border bg-muted/50 px-4 py-3">
						<span class="text-sm font-medium text-foreground"
							>เลือกแล้ว {selectedAppIds.length} ราย</span
						>
						<div class="flex gap-2 ml-auto">
							<Button
								size="sm"
								variant="outline"
								onclick={() => {
									bulkStatus = 'accepted';
									showBulkDialog = true;
								}}
								class="border-green-300 text-green-700 hover:bg-green-50"
							>
								<CheckCircle2 class="mr-1.5 h-3.5 w-3.5" />
								อนุมัติ
							</Button>
							<Button
								size="sm"
								variant="outline"
								onclick={() => {
									bulkStatus = 'rejected';
									showBulkDialog = true;
								}}
								class="border-red-300 text-red-700 hover:bg-red-50"
							>
								<XCircle class="mr-1.5 h-3.5 w-3.5" />
								ปฏิเสธ
							</Button>
							<Button
								size="sm"
								variant="outline"
								onclick={() => {
									bulkStatus = 'reviewing';
									showBulkDialog = true;
								}}
							>
								เปลี่ยนสถานะ
							</Button>
						</div>
					</div>
				{/if}

				<!-- Applications Table -->
				<div class="rounded-md border bg-card overflow-x-auto">
					<table class="w-full text-sm">
						<thead>
							<tr class="border-b bg-muted/50">
								<th class="py-3 pl-4 pr-2 w-10">
									<Checkbox
										checked={selectedAppIds.length === applications.length &&
											applications.length > 0}
										onCheckedChange={toggleAllApps}
									/>
								</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">เลขที่</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">ชื่อ-นามสกุล</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">ระดับชั้น</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">ผู้ปกครอง</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">วันสมัคร</th>
								<th class="py-3 px-3 text-left font-medium text-muted-foreground">สถานะ</th>
								<th class="py-3 px-3 text-right font-medium text-muted-foreground">จัดการ</th>
							</tr>
						</thead>
						<tbody>
							{#each applications as app (app.id)}
								<tr class="border-b hover:bg-muted/30 transition-colors">
									<td class="py-3 pl-4 pr-2">
										<Checkbox
											checked={selectedAppIds.includes(app.id)}
											onCheckedChange={() => toggleApp(app.id)}
										/>
									</td>
									<td class="py-3 px-3 font-mono text-xs text-muted-foreground"
										>{app.application_number}</td
									>
									<td class="py-3 px-3">
										<p class="font-medium">
											{app.applicant_title || ''}{app.applicant_first_name}
											{app.applicant_last_name}
										</p>
										{#if app.previous_school}
											<p class="text-xs text-muted-foreground">{app.previous_school}</p>
										{/if}
									</td>
									<td class="py-3 px-3 text-sm">{app.grade_level_name || '-'}</td>
									<td class="py-3 px-3">
										{#if app.guardian_name}
											<p class="text-sm">{app.guardian_name}</p>
											{#if app.guardian_phone}
												<p class="text-xs text-muted-foreground">{app.guardian_phone}</p>
											{/if}
										{:else}
											<span class="text-muted-foreground">-</span>
										{/if}
									</td>
									<td class="py-3 px-3 text-sm text-muted-foreground">
										{#if app.submitted_at}
											{formatDate(app.submitted_at)}
										{:else}
											<span class="italic">ยังไม่ส่ง</span>
										{/if}
									</td>
									<td class="py-3 px-3">
										<span
											class="rounded-full border px-2 py-0.5 text-xs font-medium {APPLICATION_STATUS_COLORS[
												app.status
											]}"
										>
											{APPLICATION_STATUS_LABELS[app.status]}
										</span>
									</td>
									<td class="py-3 px-3 text-right">
										<div class="flex justify-end gap-1">
											<Button
												href="/staff/academic/admission/{periodId}/applications/{app.id}"
												variant="ghost"
												size="sm"
											>
												<Eye class="h-3.5 w-3.5" />
											</Button>
										</div>
									</td>
								</tr>
							{/each}
							{#if applications.length === 0}
								<tr>
									<td colspan={8} class="py-16 text-center text-muted-foreground">
										<ClipboardList class="mx-auto mb-3 h-10 w-10 opacity-30" />
										<p>ยังไม่มีใบสมัคร</p>
									</td>
								</tr>
							{/if}
						</tbody>
					</table>
				</div>

				<!-- Pagination -->
				{#if totalPages > 1}
					<div class="flex items-center justify-between">
						<span class="text-sm text-muted-foreground">
							แสดง {applications.length} จาก {totalApps} รายการ
						</span>
						<div class="flex gap-2">
							<Button
								variant="outline"
								size="sm"
								disabled={currentPage === 1}
								onclick={() => {
									currentPage--;
									loadApplications();
								}}>← ก่อนหน้า</Button
							>
							<span class="flex items-center px-3 text-sm">
								หน้า {currentPage} / {totalPages}
							</span>
							<Button
								variant="outline"
								size="sm"
								disabled={currentPage >= totalPages}
								onclick={() => {
									currentPage++;
									loadApplications();
								}}>ถัดไป →</Button
							>
						</div>
					</div>
				{/if}

				<!-- Add application shortcut -->
				<div class="flex justify-center pt-2">
					<Button
						href="/staff/academic/admission/{periodId}/applications/new"
						variant="outline"
						class="gap-2"
					>
						<ClipboardList class="h-4 w-4" />
						เพิ่มใบสมัครด้วยตนเอง
					</Button>
				</div>
			</Tabs.Content>

			<!-- ==================== TAB: Selections ==================== -->
			<Tabs.Content value="selections" class="space-y-4 mt-4">
				<div class="flex flex-col gap-3 sm:flex-row sm:justify-between sm:items-center">
					<p class="text-sm text-muted-foreground">
						รายชื่อผู้ผ่านการคัดเลือก {selections.filter((s) => s.selection_type === 'main').length} ราย
						(สำรอง {selections.filter((s) => s.selection_type === 'waitlist').length} ราย)
					</p>
					<div class="flex gap-2">
						<Button
							variant="outline"
							size="sm"
							onclick={() => (showGenerateDialog = true)}
							class="gap-2"
							disabled={selections.filter((s) => s.is_confirmed && !s.student_user_id).length === 0}
						>
							<Sparkles class="h-4 w-4" />
							สร้าง Student Account
						</Button>
					</div>
				</div>

				{#if selections.length === 0}
					<div
						class="flex h-48 flex-col items-center justify-center rounded-md border border-dashed text-muted-foreground"
					>
						<UserCheck class="mb-3 h-10 w-10 opacity-30" />
						<p>ยังไม่มีรายชื่อผู้ผ่านคัดเลือก</p>
						<p class="mt-1 text-xs">อนุมัติใบสมัครก่อนแล้วจัดทำรายชื่อผู้ผ่านการคัดเลือก</p>
					</div>
				{:else}
					<div class="rounded-md border bg-card overflow-x-auto">
						<table class="w-full text-sm">
							<thead>
								<tr class="border-b bg-muted/50">
									<th class="py-3 px-4 text-left font-medium text-muted-foreground w-12">ลำดับ</th>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">ชื่อ-นามสกุล</th
									>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground"
										>เลขที่ใบสมัคร</th
									>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">ระดับชั้น</th>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">คะแนน</th>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">ประเภท</th>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">ยืนยันสิทธิ์</th
									>
									<th class="py-3 px-3 text-left font-medium text-muted-foreground">Account</th>
									<th class="py-3 px-3 text-right font-medium text-muted-foreground">จัดการ</th>
								</tr>
							</thead>
							<tbody>
								{#each selections as sel (sel.id)}
									<tr class="border-b hover:bg-muted/30 transition-colors">
										<td class="py-3 px-4 font-mono text-center font-medium">{sel.rank || '-'}</td>
										<td class="py-3 px-3 font-medium">{sel.applicant_name || '-'}</td>
										<td class="py-3 px-3 font-mono text-xs text-muted-foreground"
											>{sel.application_number || '-'}</td
										>
										<td class="py-3 px-3 text-sm">{sel.applying_grade_level_name || '-'}</td>
										<td class="py-3 px-3">
											{#if sel.total_score != null}
												<span class="font-mono font-semibold">{sel.total_score}</span>
											{:else}
												<span class="text-muted-foreground">-</span>
											{/if}
										</td>
										<td class="py-3 px-3">
											<span
												class="rounded-full px-2 py-0.5 text-xs font-medium {sel.selection_type ===
												'main'
													? 'bg-green-100 text-green-800'
													: 'bg-orange-100 text-orange-800'}"
											>
												{sel.selection_type === 'main' ? 'หลัก' : 'สำรอง'}
											</span>
										</td>
										<td class="py-3 px-3">
											{#if sel.is_confirmed}
												<span class="flex items-center gap-1 text-xs text-emerald-600 font-medium">
													<CheckCircle2 class="h-3.5 w-3.5" /> ยืนยันแล้ว
												</span>
											{:else}
												<span class="text-xs text-muted-foreground">รอยืนยัน</span>
											{/if}
										</td>
										<td class="py-3 px-3">
											{#if sel.student_user_id}
												<span class="flex items-center gap-1 text-xs text-blue-600 font-medium">
													<Users class="h-3.5 w-3.5" /> สร้างแล้ว
												</span>
											{:else}
												<span class="text-xs text-muted-foreground">-</span>
											{/if}
										</td>
										<td class="py-3 px-3 text-right">
											{#if !sel.is_confirmed}
												<Button
													size="sm"
													variant="outline"
													class="border-emerald-300 text-emerald-700 hover:bg-emerald-50 text-xs gap-1"
													onclick={() => handleConfirmSelection(sel.id)}
												>
													<CheckCircle2 class="h-3.5 w-3.5" />
													ยืนยัน
												</Button>
											{/if}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</Tabs.Content>
		</Tabs.Root>
	{/if}
</div>

<!-- Bulk Status Dialog -->
<Dialog.Root bind:open={showBulkDialog}>
	<Dialog.Content class="sm:max-w-[480px]">
		<Dialog.Header>
			<Dialog.Title>เปลี่ยนสถานะ {selectedAppIds.length} ใบสมัคร</Dialog.Title>
		</Dialog.Header>
		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<Label>สถานะใหม่</Label>
				<Select.Root type="single" bind:value={bulkStatus}>
					<Select.Trigger class="w-full">{APPLICATION_STATUS_LABELS[bulkStatus]}</Select.Trigger>
					<Select.Content>
						{#each Object.entries(APPLICATION_STATUS_LABELS) as [val, label]}
							<Select.Item value={val}>{label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-2">
				<Label>หมายเหตุ (ไม่บังคับ)</Label>
				<Textarea bind:value={bulkNotes} placeholder="บันทึกเหตุผลหรือหมายเหตุ..." rows={3} />
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showBulkDialog = false)}>ยกเลิก</Button>
			<Button onclick={executeBulkStatus} disabled={bulkSubmitting}>
				{#if bulkSubmitting}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				ยืนยัน
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Generate Students Dialog -->
<Dialog.Root bind:open={showGenerateDialog}>
	<Dialog.Content class="sm:max-w-[480px]">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<Sparkles class="h-5 w-5 text-primary" />
				สร้าง Student Account
			</Dialog.Title>
			<Dialog.Description>
				ระบบจะสร้าง User Account และบัญชีนักเรียนสำหรับผู้ที่ยืนยันสิทธิ์แล้วโดยอัตโนมัติ
			</Dialog.Description>
		</Dialog.Header>
		<div class="py-4">
			{#if generateResult}
				<div class="rounded-lg bg-emerald-50 p-4 text-center dark:bg-emerald-950/20">
					<p class="font-semibold text-emerald-700 dark:text-emerald-400">
						{generateResult.message}
					</p>
					<div class="mt-2 flex justify-center gap-6">
						<div>
							<p class="text-2xl font-bold text-emerald-600">{generateResult.created_count}</p>
							<p class="text-xs text-muted-foreground">สร้างแล้ว</p>
						</div>
						<div>
							<p class="text-2xl font-bold text-orange-500">{generateResult.skipped_count}</p>
							<p class="text-xs text-muted-foreground">ข้ามไป</p>
						</div>
					</div>
				</div>
			{:else}
				<div class="rounded-lg bg-muted/50 p-4">
					<p class="text-sm text-muted-foreground">
						จำนวนที่ยืนยันสิทธิ์แล้วและยังไม่มี account:
						<strong class="text-foreground">
							{selections.filter((s) => s.is_confirmed && !s.student_user_id).length} ราย
						</strong>
					</p>
				</div>
			{/if}
		</div>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showGenerateDialog = false;
					generateResult = null;
				}}
			>
				{generateResult ? 'ปิด' : 'ยกเลิก'}
			</Button>
			{#if !generateResult}
				<Button onclick={handleGenerateStudents} disabled={generating} class="gap-2">
					{#if generating}
						<Loader2 class="h-4 w-4 animate-spin" />
					{:else}
						<Sparkles class="h-4 w-4" />
					{/if}
					สร้าง Account
				</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
