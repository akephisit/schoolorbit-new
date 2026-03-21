<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		listApplications,
		verifyApplication,
		rejectApplication,
		deleteApplication,
		unverifyApplication,
		type ApplicationListItem,
		applicationStatusLabel,
		applicationStatusColor
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Table from '$lib/components/ui/table';
	import { Textarea } from '$lib/components/ui/textarea';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Search, Check, X, Eye, Users, Filter, LoaderCircle, Trash2, RotateCcw } from 'lucide-svelte';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';

	let { data, params }: PageProps = $props();

	let id = $derived(params.id);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let search = $state('');
	let statusFilter = $state('');
	let dateFilter = $state('');

	const displayedApps = $derived(
		dateFilter
			? applications.filter((a) => a.createdAt?.slice(0, 10) === dateFilter)
			: applications
	);

	let showRejectDialog = $state(false);
	let rejectingApp: ApplicationListItem | null = $state(null);
	let rejectReason = $state('');
	let rejecting = $state(false);

	let showDeleteDialog = $state(false);
	let deletingApp: ApplicationListItem | null = $state(null);
	let deleting = $state(false);

	let showUnverifyDialog = $state(false);
	let unverifyingApp: ApplicationListItem | null = $state(null);
	let unverifying = $state(false);

	const statusVariant: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
		submitted: 'secondary',
		verified: 'default',
		rejected: 'destructive',
		accepted: 'default',
		enrolled: 'default',
		withdrawn: 'outline'
	};

	async function loadApps() {
		if (!id) return;
		loading = true;
		try {
			applications = await listApplications(id, {
				status: statusFilter || undefined,
				search: search || undefined
			});
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleVerify(app: ApplicationListItem) {
		try {
			await verifyApplication(app.id);
			toast.success(`ยืนยัน ${app.fullName} แล้ว`);
			applications = applications.map(a => a.id === app.id ? { ...a, status: 'verified' } : a);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยืนยันไม่สำเร็จ');
		}
	}

	async function handleRejectConfirm() {
		if (!rejectingApp || !rejectReason.trim()) return;
		rejecting = true;
		try {
			await rejectApplication(rejectingApp.id, rejectReason);
			toast.success('ปฏิเสธใบสมัครแล้ว');
			applications = applications.map(a => a.id === rejectingApp!.id ? { ...a, status: 'rejected' } : a);
			showRejectDialog = false;
			rejectingApp = null;
			rejectReason = '';
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ปฏิเสธไม่สำเร็จ');
		} finally {
			rejecting = false;
		}
	}

	async function handleDeleteConfirm() {
		if (!deletingApp) return;
		deleting = true;
		try {
			await deleteApplication(deletingApp.id);
			toast.success(`ลบใบสมัครของ ${deletingApp.fullName} แล้ว`);
			applications = applications.filter(a => a.id !== deletingApp!.id);
			showDeleteDialog = false;
			deletingApp = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}

	async function handleUnverifyConfirm() {
		if (!unverifyingApp) return;
		unverifying = true;
		try {
			await unverifyApplication(unverifyingApp.id);
			toast.success(`ยกเลิกการอนุมัติ ${unverifyingApp.fullName} แล้ว`);
			applications = applications.map(a => a.id === unverifyingApp!.id ? { ...a, status: 'submitted' } : a);
			showUnverifyDialog = false;
			unverifyingApp = null;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ยกเลิกการอนุมัติไม่สำเร็จ');
		} finally {
			unverifying = false;
		}
	}

	function saveNavContext(apps: ApplicationListItem[]) {
		sessionStorage.setItem('appNavIds', JSON.stringify(apps.map((a) => a.id)));
	}

	onMount(loadApps);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-5">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<Users class="w-6 h-6" /> ใบสมัคร
		</h1>
	</div>

	<!-- Filters -->
	<Card.Root>
		<Card.Content class="pt-4 pb-4">
			<div class="flex flex-col sm:flex-row flex-wrap gap-2">
				<div class="relative flex-1 min-w-0 sm:min-w-48">
					<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
					<Input
						bind:value={search}
						placeholder="ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร..."
						class="pl-9"
						onkeypress={(e: KeyboardEvent) => e.key === 'Enter' && loadApps()}
					/>
				</div>
				<div class="w-full sm:w-44">
					<Select.Root type="single" bind:value={statusFilter} onValueChange={loadApps}>
						<Select.Trigger>
							{statusFilter ? applicationStatusLabel[statusFilter] : 'สถานะทั้งหมด'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="">สถานะทั้งหมด</Select.Item>
							<Select.Item value="submitted">รอตรวจสอบ</Select.Item>
							<Select.Item value="verified">ผ่านตรวจสอบ</Select.Item>
							<Select.Item value="rejected">ไม่ผ่าน</Select.Item>
							<Select.Item value="accepted">ได้รับคัดเลือก</Select.Item>
							<Select.Item value="enrolled">มอบตัวแล้ว</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
				<div class="flex items-center gap-1.5 flex-1 sm:flex-none sm:w-48">
					<DatePicker bind:value={dateFilter} placeholder="กรองตามวันที่" class="w-full" />
					{#if dateFilter}
						<Button variant="ghost" size="icon" class="h-9 w-9 shrink-0" onclick={() => (dateFilter = '')} title="ล้างวันที่">
							<X class="w-3.5 h-3.5" />
						</Button>
					{/if}
				</div>
				<Button onclick={loadApps} variant="outline" size="sm" class="gap-1.5 w-full sm:w-auto">
					<Filter class="w-4 h-4" /> ค้นหา
				</Button>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Table -->
	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if applications.length === 0}
		<Card.Root>
			<Card.Content class="flex flex-col items-center py-16 gap-3 text-muted-foreground">
				<Users class="w-12 h-12 opacity-40" />
				<p>ไม่พบใบสมัคร</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<Card.Root>
			<div class="overflow-x-auto">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="w-24">เลขที่</Table.Head>
						<Table.Head>ชื่อ-สกุล</Table.Head>
						<Table.Head>เลขบัตร</Table.Head>
						<Table.Head>สาย</Table.Head>
						<Table.Head>สถานะ</Table.Head>
						<Table.Head class="text-right">จัดการ</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each displayedApps as app (app.id)}
						<Table.Row>
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell>
								<p class="font-medium">{app.fullName}</p>
								<p class="text-xs text-muted-foreground">{app.phone ?? ''}</p>
							</Table.Cell>
							<Table.Cell class="font-mono text-xs text-muted-foreground"
								>{app.nationalId}</Table.Cell
							>
							<Table.Cell class="text-sm">{app.trackName ?? '-'}</Table.Cell>
							<Table.Cell>
								<Badge variant={statusVariant[app.status] ?? 'outline'}>
									{applicationStatusLabel[app.status] ?? app.status}
								</Badge>
							</Table.Cell>
							<Table.Cell class="text-right">
								<div class="flex justify-end gap-1">
									<Button
										href="/staff/academic/admission/{id}/applications/{app.id}"
										variant="ghost"
										size="icon"
										class="h-8 w-8"
										onclick={() => saveNavContext(displayedApps)}
									>
										<Eye class="w-3.5 h-3.5" />
									</Button>
									{#if app.status === 'submitted'}
										<Button
											variant="ghost"
											size="icon"
											class="h-8 w-8 text-green-600 hover:text-green-700"
											onclick={() => handleVerify(app)}
											title="อนุมัติ"
										>
											<Check class="w-3.5 h-3.5" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											class="h-8 w-8 text-destructive hover:text-destructive"
											onclick={() => {
												rejectingApp = app;
												showRejectDialog = true;
											}}
											title="ไม่อนุมัติ"
										>
											<X class="w-3.5 h-3.5" />
										</Button>
									{/if}
									{#if app.status === 'verified'}
										<Button
											variant="ghost"
											size="icon"
											class="h-8 w-8 text-muted-foreground hover:text-destructive"
											onclick={() => { unverifyingApp = app; showUnverifyDialog = true; }}
											title="ยกเลิกการอนุมัติ"
										>
											<RotateCcw class="w-3.5 h-3.5" />
										</Button>
									{/if}
								<Button
									variant="ghost"
									size="icon"
									class="h-8 w-8 text-muted-foreground hover:text-destructive"
									onclick={() => { deletingApp = app; showDeleteDialog = true; }}
									title="ลบใบสมัคร"
								>
									<Trash2 class="w-3.5 h-3.5" />
								</Button>
								</div>
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
			</div>

			<div class="px-4 py-3 border-t border-border">
				<p class="text-xs text-muted-foreground">แสดง {displayedApps.length} จาก {applications.length} รายการ</p>
			</div>
		</Card.Root>
	{/if}
</div>

<!-- Reject Dialog -->
<Dialog.Root bind:open={showRejectDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ปฏิเสธใบสมัคร</Dialog.Title>
			<Dialog.Description>
				ปฏิเสธใบสมัครของ <strong>{rejectingApp?.fullName}</strong>
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-2 py-2">
			<Label for="reject-reason">เหตุผล <span class="text-destructive">*</span></Label>
			<Textarea
				id="reject-reason"
				bind:value={rejectReason}
				placeholder="ระบุเหตุผลที่ปฏิเสธ..."
				rows={3}
			/>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showRejectDialog = false)}>ยกเลิก</Button>
			<Button
				variant="destructive"
				onclick={handleRejectConfirm}
				disabled={rejecting || !rejectReason.trim()}
			>
				{#if rejecting}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
				{rejecting ? 'กำลังดำเนินการ...' : 'ปฏิเสธ'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Unverify Dialog -->
<Dialog.Root bind:open={showUnverifyDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ยกเลิกการอนุมัติ</Dialog.Title>
			<Dialog.Description>
				ยืนยันการยกเลิกการอนุมัติใบสมัครของ <strong>{unverifyingApp?.fullName}</strong>?
				ใบสมัครจะกลับสู่สถานะ "รอตรวจสอบ"
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showUnverifyDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleUnverifyConfirm} disabled={unverifying}>
				{#if unverifying}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
				{unverifying ? 'กำลังดำเนินการ...' : 'ยืนยัน'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Dialog -->
<Dialog.Root bind:open={showDeleteDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ลบใบสมัคร</Dialog.Title>
			<Dialog.Description>
				ลบใบสมัครของ <strong>{deletingApp?.fullName}</strong> ออกจากระบบ การดำเนินการนี้ไม่สามารถยกเลิกได้
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
			<Button variant="destructive" onclick={handleDeleteConfirm} disabled={deleting}>
				{#if deleting}<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />{/if}
				{deleting ? 'กำลังลบ...' : 'ยืนยันการลบ'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
