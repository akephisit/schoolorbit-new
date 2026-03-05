<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listCheckins,
		getCheckinStats,
		confirmCheckin,
		markAbsent,
		type AdmissionSelection,
		type CheckinStats,
		type CheckinStatus,
		CHECKIN_STATUS_LABELS,
		CHECKIN_STATUS_COLORS
	} from '$lib/api/admission';
	import { toast } from 'svelte-sonner';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import ArrowLeft from 'lucide-svelte/icons/arrow-left';
	import Loader2 from 'lucide-svelte/icons/loader-2';
	import Search from 'lucide-svelte/icons/search';
	import UserCheck from 'lucide-svelte/icons/user-check';
	import UserX from 'lucide-svelte/icons/user-x';
	import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
	import Clock from 'lucide-svelte/icons/clock';
	import XCircle from 'lucide-svelte/icons/x-circle';
	import Key from 'lucide-svelte/icons/key';
	import Copy from 'lucide-svelte/icons/copy';
	import ClipboardCheck from 'lucide-svelte/icons/clipboard-check';

	let { data } = $props();
	const { periodId } = data;

	let stats = $state<CheckinStats | null>(null);
	let items = $state<AdmissionSelection[]>([]);
	let loading = $state(true);
	let search = $state('');
	let filterStatus = $state<CheckinStatus | 'all'>('all');

	// Confirm dialog
	let confirmItem = $state<AdmissionSelection | null>(null);
	let confirmNotes = $state('');
	let confirming = $state(false);

	// Result dialog (แสดง username/password หลัง checkin)
	let resultData = $state<{ username: string; password: string; student_id: string } | null>(null);

	// Absent dialog
	let absentItem = $state<AdmissionSelection | null>(null);
	let absentNotes = $state('');
	let markingAbsent = $state(false);

	const filteredItems = $derived(
		items.filter((item) => {
			if (filterStatus !== 'all' && item.checkin_status !== filterStatus) return false;
			if (search) {
				const q = search.toLowerCase();
				return (
					(item.applicant_name ?? '').toLowerCase().includes(q) ||
					(item.application_number ?? '').toLowerCase().includes(q) ||
					(item.guardian_phone ?? '').includes(q)
				);
			}
			return true;
		})
	);

	async function loadData() {
		loading = true;
		try {
			const [checkinsRes, statsRes] = await Promise.all([
				listCheckins(periodId),
				getCheckinStats(periodId)
			]);
			items = checkinsRes.data;
			stats = statsRes.data;
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleConfirm() {
		if (!confirmItem) return;
		confirming = true;
		try {
			const result = await confirmCheckin(confirmItem.id, confirmNotes || undefined);
			resultData = {
				username: result.username,
				password: result.password,
				student_id: result.student_id
			};
			confirmItem = null;
			confirmNotes = '';
			toast.success('รายงานตัวและสร้างบัญชีเรียบร้อยแล้ว');
			await loadData();
		} catch (e: any) {
			toast.error(e.message || 'รายงานตัวไม่สำเร็จ');
		} finally {
			confirming = false;
		}
	}

	async function handleMarkAbsent() {
		if (!absentItem) return;
		markingAbsent = true;
		try {
			await markAbsent(absentItem.id, absentNotes || undefined);
			toast.success('บันทึกไม่มารายงานตัวแล้ว');
			absentItem = null;
			absentNotes = '';
			await loadData();
		} catch (e: any) {
			toast.error(e.message || 'บันทึกไม่สำเร็จ');
		} finally {
			markingAbsent = false;
		}
	}

	function copyToClipboard(text: string) {
		navigator.clipboard.writeText(text);
		toast.success('คัดลอกแล้ว');
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" href="/staff/academic/admission/{periodId}">
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="flex items-center gap-2 text-xl font-bold">
				<ClipboardCheck class="h-5 w-5 text-primary" />
				รายงานตัวนักเรียน
			</h1>
			<p class="text-sm text-muted-foreground">กดยืนยันรายงานตัว → ระบบสร้าง account ทันที</p>
		</div>
	</div>

	<!-- Stats -->
	{#if stats}
		<div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
			<Card.Root class="border-2 border-border">
				<Card.Content class="pt-5 pb-4 text-center">
					<div class="text-3xl font-bold">{stats.total_confirmed}</div>
					<div class="text-xs text-muted-foreground mt-1">ยืนยันสิทธิ์ทั้งหมด</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="border-2 border-yellow-200 bg-yellow-50">
				<Card.Content class="pt-5 pb-4 text-center">
					<div class="text-3xl font-bold text-yellow-700">{stats.pending_checkin}</div>
					<div class="text-xs text-yellow-600 mt-1">รอรายงานตัว</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="border-2 border-green-200 bg-green-50">
				<Card.Content class="pt-5 pb-4 text-center">
					<div class="text-3xl font-bold text-green-700">{stats.checked_in}</div>
					<div class="text-xs text-green-600 mt-1">รายงานตัวแล้ว</div>
				</Card.Content>
			</Card.Root>
			<Card.Root class="border-2 border-red-200 bg-red-50">
				<Card.Content class="pt-5 pb-4 text-center">
					<div class="text-3xl font-bold text-red-700">{stats.absent}</div>
					<div class="text-xs text-red-600 mt-1">ไม่มา</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}

	<!-- Search + Filter -->
	<div class="flex gap-3">
		<div class="relative flex-1">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
			<Input
				bind:value={search}
				placeholder="ค้นหาชื่อ เลขใบสมัคร หรือเบอร์โทรผู้ปกครอง..."
				class="pl-9"
			/>
		</div>
		<Select.Root type="single" bind:value={filterStatus}>
			<Select.Trigger class="w-44">
				{filterStatus === 'all' ? 'ทุกสถานะ' : CHECKIN_STATUS_LABELS[filterStatus as CheckinStatus]}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกสถานะ</Select.Item>
				<Select.Item value="pending">รอรายงานตัว</Select.Item>
				<Select.Item value="checked_in">รายงานตัวแล้ว</Select.Item>
				<Select.Item value="absent">ไม่มา</Select.Item>
			</Select.Content>
		</Select.Root>
	</div>

	<!-- List -->
	{#if loading}
		<div class="flex h-48 items-center justify-center">
			<Loader2 class="h-8 w-8 animate-spin text-primary" />
		</div>
	{:else if filteredItems.length === 0}
		<Card.Root>
			<Card.Content class="py-16 text-center text-muted-foreground">
				{search ? 'ไม่พบผลการค้นหา' : 'ยังไม่มีรายชื่อผู้ยืนยันสิทธิ์'}
			</Card.Content>
		</Card.Root>
	{:else}
		<div class="space-y-2">
			{#each filteredItems as item}
				<Card.Root
					class="transition-all {item.checkin_status === 'checked_in' ? 'opacity-70' : ''}"
				>
					<Card.Content class="flex items-center gap-4 px-5 py-4">
						<!-- Status Icon -->
						<div class="flex-shrink-0">
							{#if item.checkin_status === 'checked_in'}
								<CheckCircle2 class="h-8 w-8 text-green-500" />
							{:else if item.checkin_status === 'absent'}
								<XCircle class="h-8 w-8 text-red-400" />
							{:else}
								<Clock class="h-8 w-8 text-yellow-500" />
							{/if}
						</div>

						<!-- Info -->
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 flex-wrap">
								<span class="font-semibold">{item.applicant_name}</span>
								<span class="text-xs text-muted-foreground">{item.application_number}</span>
								<span
									class="rounded-full px-2 py-0.5 text-xs font-medium {CHECKIN_STATUS_COLORS[
										item.checkin_status as CheckinStatus
									]}"
								>
									{CHECKIN_STATUS_LABELS[item.checkin_status as CheckinStatus]}
								</span>
							</div>
							<div class="mt-1 flex flex-wrap gap-x-4 gap-y-0.5 text-sm text-muted-foreground">
								{#if item.applying_grade_level_name}
									<span>ระดับ: {item.applying_grade_level_name}</span>
								{/if}
								{#if item.study_plan_version_name}
									<span>สาย: {item.study_plan_version_name}</span>
								{/if}
								{#if item.classroom_name}
									<span>ห้อง: {item.classroom_name}</span>
								{/if}
								{#if item.guardian_phone}
									<span>ผู้ปกครอง: {item.guardian_phone}</span>
								{/if}
								{#if item.app_total_score !== undefined}
									<span>คะแนนรวม: {item.app_total_score}</span>
								{/if}
							</div>
							{#if item.checkin_status === 'checked_in'}
								<div class="mt-1 text-xs text-green-600">
									รายงานตัว {item.checked_in_at
										? new Date(item.checked_in_at).toLocaleString('th-TH')
										: ''} | account: {item.student_username ?? '-'}
								</div>
							{/if}
						</div>

						<!-- Actions -->
						<div class="flex flex-shrink-0 gap-2">
							{#if item.checkin_status === 'pending'}
								<Button
									size="sm"
									class="bg-green-600 hover:bg-green-700"
									onclick={() => {
										confirmItem = item;
										confirmNotes = '';
									}}
								>
									<UserCheck class="mr-1.5 h-3.5 w-3.5" />
									ยืนยันรายงานตัว
								</Button>
								<Button
									size="sm"
									variant="outline"
									class="border-red-200 text-red-600 hover:bg-red-50"
									onclick={() => {
										absentItem = item;
										absentNotes = '';
									}}
								>
									<UserX class="mr-1.5 h-3.5 w-3.5" />
									ไม่มา
								</Button>
							{/if}
						</div>
					</Card.Content>
				</Card.Root>
			{/each}
		</div>
	{/if}
</div>

<!-- Confirm Checkin Dialog -->
<Dialog.Root
	open={!!confirmItem}
	onOpenChange={(o) => {
		if (!o) confirmItem = null;
	}}
>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<UserCheck class="h-5 w-5 text-green-600" />
				ยืนยันการรายงานตัว
			</Dialog.Title>
			<Dialog.Description>ระบบจะสร้าง account นักเรียนทันทีหลังยืนยัน</Dialog.Description>
		</Dialog.Header>
		{#if confirmItem}
			<div class="space-y-4">
				<div class="rounded-xl bg-muted/40 p-4 space-y-2 text-sm">
					<div class="text-lg font-bold">{confirmItem.applicant_name}</div>
					<div class="text-muted-foreground">{confirmItem.application_number}</div>
					{#if confirmItem.applying_grade_level_name}
						<div>
							ระดับที่สมัคร: <span class="font-medium">{confirmItem.applying_grade_level_name}</span
							>
						</div>
					{/if}
					{#if confirmItem.study_plan_version_name}
						<div>
							สายการเรียน: <span class="font-medium text-primary"
								>{confirmItem.study_plan_version_name}</span
							>
						</div>
					{/if}
					{#if confirmItem.classroom_name}
						<div>ห้องเรียน: <span class="font-medium">{confirmItem.classroom_name}</span></div>
					{/if}
					{#if confirmItem.guardian_phone}
						<div>ผู้ปกครอง: {confirmItem.guardian_name} ({confirmItem.guardian_phone})</div>
					{/if}
				</div>
				<div class="grid gap-2">
					<Label>บันทึกเพิ่มเติม (ถ้ามี)</Label>
					<Textarea
						bind:value={confirmNotes}
						placeholder="เช่น นำเอกสารมาครบ, ชำระค่าธรรมเนียมแล้ว..."
						rows={2}
					/>
				</div>
			</div>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (confirmItem = null)}>ยกเลิก</Button>
			<Button class="bg-green-600 hover:bg-green-700" onclick={handleConfirm} disabled={confirming}>
				{#if confirming}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				ยืนยันรายงานตัว
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Result Dialog -->
<Dialog.Root
	open={!!resultData}
	onOpenChange={(o) => {
		if (!o) resultData = null;
	}}
>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-green-700">
				<CheckCircle2 class="h-5 w-5" />
				สร้าง Account เรียบร้อย!
			</Dialog.Title>
			<Dialog.Description>แจ้งข้อมูลนี้ให้นักเรียน/ผู้ปกครอง</Dialog.Description>
		</Dialog.Header>
		{#if resultData}
			<div class="space-y-3">
				{#each [{ label: 'รหัสนักเรียน', value: resultData.student_id }, { label: 'Username', value: resultData.username }, { label: 'Password', value: resultData.password }] as item}
					<div class="flex items-center justify-between rounded-lg border bg-muted/30 px-4 py-3">
						<div>
							<div class="text-xs text-muted-foreground">{item.label}</div>
							<div class="font-mono font-bold tracking-wide">{item.value}</div>
						</div>
						<Button
							variant="ghost"
							size="icon"
							class="h-7 w-7"
							onclick={() => copyToClipboard(item.value)}
						>
							<Copy class="h-3.5 w-3.5" />
						</Button>
					</div>
				{/each}
				<p class="text-xs text-muted-foreground">
					⚠️ Password นี้จะแสดงเพียงครั้งเดียว กรุณาแจ้งนักเรียนก่อนปิด
				</p>
			</div>
		{/if}
		<Dialog.Footer>
			<Button onclick={() => (resultData = null)}>รับทราบ</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Mark Absent Dialog -->
<Dialog.Root
	open={!!absentItem}
	onOpenChange={(o) => {
		if (!o) absentItem = null;
	}}
>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2 text-red-700">
				<UserX class="h-5 w-5" />
				บันทึกไม่มารายงานตัว
			</Dialog.Title>
		</Dialog.Header>
		{#if absentItem}
			<div class="space-y-3">
				<div class="rounded-lg bg-muted/40 px-4 py-3">
					<div class="font-medium">{absentItem.applicant_name}</div>
					<div class="text-sm text-muted-foreground">{absentItem.application_number}</div>
				</div>
				<div class="grid gap-2">
					<Label>เหตุผล</Label>
					<Textarea
						bind:value={absentNotes}
						placeholder="เช่น ไม่มาตามกำหนด ไม่ติดต่อกลับ..."
						rows={2}
					/>
				</div>
			</div>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (absentItem = null)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleMarkAbsent} disabled={markingAbsent}>
				{#if markingAbsent}<Loader2 class="mr-2 h-4 w-4 animate-spin" />{/if}
				บันทึกไม่มา
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
