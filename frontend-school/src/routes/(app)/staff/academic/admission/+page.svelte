<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listAdmissionPeriods,
		deleteAdmissionPeriod,
		updateAdmissionPeriod,
		PERIOD_STATUS_LABELS,
		PERIOD_STATUS_COLORS,
		type AdmissionPeriod,
		type AdmissionStatus
	} from '$lib/api/admission';
	import { getAcademicStructure, type AcademicStructureData } from '$lib/api/academic';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import Plus from 'lucide-svelte/icons/plus';
	import ClipboardList from 'lucide-svelte/icons/clipboard-list';
	import Users from 'lucide-svelte/icons/users';
	import CalendarDays from 'lucide-svelte/icons/calendar-days';
	import ChevronRight from 'lucide-svelte/icons/chevron-right';
	import Pencil from 'lucide-svelte/icons/pencil';
	import Trash2 from 'lucide-svelte/icons/trash-2';
	import Eye from 'lucide-svelte/icons/eye';
	import TrendingUp from 'lucide-svelte/icons/trending-up';

	let { data } = $props();

	let periods = $state<AdmissionPeriod[]>([]);
	let structure = $state<AcademicStructureData>({ years: [], semesters: [], levels: [] });
	let loading = $state(true);
	let selectedYearId = $state('');

	// Delete dialog
	let showDeleteDialog = $state(false);
	let periodToDelete = $state<AdmissionPeriod | null>(null);
	let deleting = $state(false);

	// Status change
	let updatingStatus = $state<string | null>(null);

	const statusOptions: { value: AdmissionStatus; label: string }[] = [
		{ value: 'draft', label: 'ร่าง' },
		{ value: 'open', label: 'เปิดรับสมัคร' },
		{ value: 'closed', label: 'ปิดรับสมัคร' },
		{ value: 'announced', label: 'ประกาศผลแล้ว' },
		{ value: 'done', label: 'เสร็จสิ้น' }
	];

	async function loadData() {
		try {
			loading = true;
			const [strRes, periodsRes] = await Promise.all([
				getAcademicStructure(),
				listAdmissionPeriods()
			]);
			structure = strRes.data;
			periods = periodsRes.data;

			const activeYear = structure.years.find((y) => y.is_active) || structure.years[0];
			if (activeYear) selectedYearId = activeYear.id;
		} catch (e) {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

	async function loadPeriods() {
		try {
			const res = await listAdmissionPeriods(
				selectedYearId ? { academic_year_id: selectedYearId } : {}
			);
			periods = res.data;
		} catch {
			toast.error('โหลดรายการไม่สำเร็จ');
		}
	}

	async function changeStatus(period: AdmissionPeriod, newStatus: AdmissionStatus) {
		updatingStatus = period.id;
		try {
			await updateAdmissionPeriod(period.id, { status: newStatus });
			toast.success(`เปลี่ยนสถานะเป็น "${PERIOD_STATUS_LABELS[newStatus]}" เรียบร้อย`);
			await loadPeriods();
		} catch (e: any) {
			toast.error(e.message || 'เปลี่ยนสถานะไม่สำเร็จ');
		} finally {
			updatingStatus = null;
		}
	}

	async function confirmDelete() {
		if (!periodToDelete) return;
		deleting = true;
		try {
			await deleteAdmissionPeriod(periodToDelete.id);
			toast.success('ลบรอบรับสมัครเรียบร้อยแล้ว');
			showDeleteDialog = false;
			periodToDelete = null;
			await loadPeriods();
		} catch (e: any) {
			toast.error(e.message || 'ลบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}

	function formatDate(dateStr: string) {
		return new Date(dateStr).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'long',
			day: 'numeric'
		});
	}

	// Summary stats
	let totalApplications = $derived(periods.reduce((s, p) => s + (p.application_count || 0), 0));
	let totalPending = $derived(periods.reduce((s, p) => s + (p.pending_count || 0), 0));
	let totalConfirmed = $derived(periods.reduce((s, p) => s + (p.confirmed_count || 0), 0));
	let openPeriods = $derived(periods.filter((p) => p.status === 'open').length);

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
		<div>
			<h1 class="flex items-center gap-2 text-3xl font-bold text-foreground">
				<ClipboardList class="h-8 w-8 text-primary" />
				ระบบรับสมัครนักเรียน
			</h1>
			<p class="mt-1 text-muted-foreground">จัดการรอบรับสมัครและใบสมัครนักเรียนใหม่</p>
		</div>
		<Button href="/staff/academic/admission/new" class="flex items-center gap-2 shadow-sm">
			<Plus class="h-4 w-4" />
			สร้างรอบรับสมัครใหม่
		</Button>
	</div>

	<!-- Summary Cards -->
	{#if !loading}
		<div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
			<Card.Root
				class="border-l-4 border-l-green-500 bg-gradient-to-br from-green-50 to-white dark:from-green-950/20 dark:to-card"
			>
				<Card.Content class="pt-6">
					<div class="flex items-center justify-between">
						<div>
							<p class="text-sm text-muted-foreground">รอบเปิดรับสมัคร</p>
							<p class="text-3xl font-bold text-green-700 dark:text-green-400">{openPeriods}</p>
						</div>
						<CalendarDays class="h-10 w-10 text-green-500 opacity-60" />
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root
				class="border-l-4 border-l-blue-500 bg-gradient-to-br from-blue-50 to-white dark:from-blue-950/20 dark:to-card"
			>
				<Card.Content class="pt-6">
					<div class="flex items-center justify-between">
						<div>
							<p class="text-sm text-muted-foreground">ใบสมัครทั้งหมด</p>
							<p class="text-3xl font-bold text-blue-700 dark:text-blue-400">{totalApplications}</p>
						</div>
						<ClipboardList class="h-10 w-10 text-blue-500 opacity-60" />
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root
				class="border-l-4 border-l-yellow-500 bg-gradient-to-br from-yellow-50 to-white dark:from-yellow-950/20 dark:to-card"
			>
				<Card.Content class="pt-6">
					<div class="flex items-center justify-between">
						<div>
							<p class="text-sm text-muted-foreground">รอพิจารณา</p>
							<p class="text-3xl font-bold text-yellow-700 dark:text-yellow-400">{totalPending}</p>
						</div>
						<TrendingUp class="h-10 w-10 text-yellow-500 opacity-60" />
					</div>
				</Card.Content>
			</Card.Root>

			<Card.Root
				class="border-l-4 border-l-emerald-500 bg-gradient-to-br from-emerald-50 to-white dark:from-emerald-950/20 dark:to-card"
			>
				<Card.Content class="pt-6">
					<div class="flex items-center justify-between">
						<div>
							<p class="text-sm text-muted-foreground">ยืนยันสิทธิ์แล้ว</p>
							<p class="text-3xl font-bold text-emerald-700 dark:text-emerald-400">
								{totalConfirmed}
							</p>
						</div>
						<Users class="h-10 w-10 text-emerald-500 opacity-60" />
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}

	<!-- Year Filter -->
	<Card.Root>
		<Card.Content class="pt-6">
			<div class="flex flex-col gap-4 sm:flex-row sm:items-end">
				<div class="space-y-1.5">
					<Label>ปีการศึกษา</Label>
					<Select.Root
						type="single"
						bind:value={selectedYearId}
						onValueChange={() => loadPeriods()}
					>
						<Select.Trigger class="w-52">
							{structure.years.find((y) => y.id === selectedYearId)?.name || 'ทุกปีการศึกษา'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">ทุกปีการศึกษา</Select.Item>
							{#each structure.years as year}
								<Select.Item value={year.id}>
									{year.name}
									{year.is_active ? '(ปัจจุบัน)' : ''}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Periods List -->
	{#if loading}
		<div class="flex h-64 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else if periods.length === 0}
		<div
			class="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed text-muted-foreground"
		>
			<ClipboardList class="mb-4 h-12 w-12 opacity-40" />
			<p class="text-lg font-medium">ยังไม่มีรอบรับสมัคร</p>
			<p class="mt-1 text-sm">เริ่มต้นด้วยการสร้างรอบรับสมัครใหม่</p>
			<Button href="/staff/academic/admission/new" class="mt-4" size="sm">
				<Plus class="mr-2 h-4 w-4" />
				สร้างรอบรับสมัคร
			</Button>
		</div>
	{:else}
		<div class="space-y-4">
			{#each periods as period (period.id)}
				<Card.Root class="overflow-hidden transition-shadow hover:shadow-md">
					<div class="flex flex-col gap-0 lg:flex-row">
						<!-- Left: Status indicator bar -->
						<div
							class="w-full lg:w-1.5 {period.status === 'open'
								? 'bg-green-500'
								: period.status === 'draft'
									? 'bg-gray-300'
									: period.status === 'closed'
										? 'bg-yellow-500'
										: period.status === 'announced'
											? 'bg-blue-500'
											: 'bg-purple-500'} shrink-0"
						></div>

						<Card.Content class="flex flex-1 flex-col gap-4 pt-5 sm:flex-row sm:items-start">
							<!-- Info -->
							<div class="flex-1">
								<div class="mb-2 flex flex-wrap items-center gap-2">
									<h3 class="text-lg font-semibold text-foreground">{period.name}</h3>
									<span
										class="rounded-full px-2.5 py-0.5 text-xs font-medium {PERIOD_STATUS_COLORS[
											period.status
										]}"
									>
										{PERIOD_STATUS_LABELS[period.status]}
									</span>
								</div>

								<div class="mb-3 flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
									<span class="flex items-center gap-1">
										<CalendarDays class="h-3.5 w-3.5" />
										{formatDate(period.open_date)} — {formatDate(period.close_date)}
									</span>
									{#if period.academic_year_name}
										<span>ปีการศึกษา {period.academic_year_name}</span>
									{/if}
									{#if period.total_capacity}
										<span>รับ {period.total_capacity} คน</span>
									{/if}
								</div>

								<!-- Stats row -->
								<div class="flex flex-wrap gap-4">
									<div class="text-center">
										<p class="text-2xl font-bold text-foreground">
											{period.application_count || 0}
										</p>
										<p class="text-xs text-muted-foreground">ใบสมัคร</p>
									</div>
									<div class="text-center">
										<p class="text-2xl font-bold text-yellow-600">{period.pending_count || 0}</p>
										<p class="text-xs text-muted-foreground">รอพิจารณา</p>
									</div>
									<div class="text-center">
										<p class="text-2xl font-bold text-green-600">{period.accepted_count || 0}</p>
										<p class="text-xs text-muted-foreground">ผ่านคัดเลือก</p>
									</div>
									<div class="text-center">
										<p class="text-2xl font-bold text-emerald-600">{period.confirmed_count || 0}</p>
										<p class="text-xs text-muted-foreground">ยืนยันสิทธิ์</p>
									</div>
								</div>
							</div>

							<!-- Actions -->
							<div class="flex flex-col gap-2 sm:items-end">
								<!-- Status Changer -->
								<div class="w-full sm:w-auto">
									{#if updatingStatus === period.id}
										<div
											class="flex h-9 w-full items-center justify-center rounded-md border px-3 text-sm sm:w-40"
										>
											<Loader2 class="h-4 w-4 animate-spin" />
										</div>
									{:else}
										<Select.Root
											type="single"
											value={period.status}
											onValueChange={(v) => changeStatus(period, v as AdmissionStatus)}
										>
											<Select.Trigger class="w-full sm:w-40 text-sm">
												{PERIOD_STATUS_LABELS[period.status]}
											</Select.Trigger>
											<Select.Content>
												{#each statusOptions as opt}
													<Select.Item value={opt.value}>{opt.label}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
									{/if}
								</div>

								<!-- Action Buttons -->
								<div class="flex gap-2">
									<Button
										href="/staff/academic/admission/{period.id}"
										size="sm"
										class="flex items-center gap-1.5"
									>
										<Eye class="h-3.5 w-3.5" />
										จัดการ
									</Button>
									<Button
										href="/staff/academic/admission/{period.id}/edit"
										variant="outline"
										size="sm"
									>
										<Pencil class="h-3.5 w-3.5" />
									</Button>
									<Button
										variant="outline"
										size="sm"
										class="text-red-500 hover:bg-red-50 hover:text-red-600"
										onclick={() => {
											periodToDelete = period;
											showDeleteDialog = true;
										}}
									>
										<Trash2 class="h-3.5 w-3.5" />
									</Button>
								</div>
							</div>
						</Card.Content>
					</div>
				</Card.Root>
			{/each}
		</div>
	{/if}
</div>

<!-- Delete Dialog -->
<Dialog.Root bind:open={showDeleteDialog}>
	<Dialog.Content class="sm:max-w-[400px]">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-red-600">
				<Trash2 class="h-5 w-5" />
				ยืนยันการลบรอบรับสมัคร
			</Dialog.Title>
			<Dialog.Description>
				{#if periodToDelete}
					คุณต้องการลบรอบ "<strong>{periodToDelete.name}</strong>" ใช่หรือไม่?
					การกระทำนี้ไม่สามารถย้อนกลับได้
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showDeleteDialog = false;
					periodToDelete = null;
				}}
			>
				ยกเลิก
			</Button>
			<Button variant="destructive" onclick={confirmDelete} disabled={deleting}>
				{#if deleting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				ลบรอบรับสมัคร
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
