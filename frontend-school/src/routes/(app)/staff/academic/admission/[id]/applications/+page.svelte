<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		listApplications,
		verifyApplication,
		rejectApplication,
		type ApplicationListItem,
		applicationStatusLabel,
		applicationStatusColor
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Search, Check, X, Eye, Users, Filter } from 'lucide-svelte';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogDescription,
		DialogFooter
	} from '$lib/components/ui/dialog';

	let id = $derived($page.params.id);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let search = $state('');
	let statusFilter = $state('');

	// Reject dialog
	let showRejectDialog = $state(false);
	let rejectingApp: ApplicationListItem | null = $state(null);
	let rejectReason = $state('');
	let rejecting = $state(false);

	let selected = $state<Set<string>>(new Set());

	async function load() {
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
			await load();
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
			showRejectDialog = false;
			rejectingApp = null;
			rejectReason = '';
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ปฏิเสธไม่สำเร็จ');
		} finally {
			rejecting = false;
		}
	}

	function formatDate(d: string) {
		return new Date(d).toLocaleDateString('th-TH', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	onMount(load);
</script>

<svelte:head>
	<title>ใบสมัคร - SchoolOrbit</title>
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
	<div class="bg-card border border-border rounded-lg p-4">
		<div class="flex flex-wrap gap-3">
			<div class="relative flex-1 min-w-48">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					bind:value={search}
					placeholder="ค้นหาชื่อ, เลขบัตร, เลขที่ใบสมัคร..."
					class="pl-9"
					onkeypress={(e) => e.key === 'Enter' && load()}
				/>
			</div>
			<select
				bind:value={statusFilter}
				onchange={load}
				class="px-3 py-2 text-sm rounded-md border border-border bg-background"
			>
				<option value="">สถานะทั้งหมด</option>
				<option value="submitted">รอตรวจสอบ</option>
				<option value="verified">ผ่านตรวจสอบ</option>
				<option value="rejected">ไม่ผ่าน</option>
				<option value="accepted">ได้รับคัดเลือก</option>
				<option value="enrolled">มอบตัวแล้ว</option>
			</select>
			<Button onclick={load} size="sm" variant="outline" class="gap-1">
				<Filter class="w-4 h-4" /> ค้นหา
			</Button>
		</div>
	</div>

	<!-- Table -->
	{#if loading}
		<div class="bg-card border border-border rounded-lg p-10 text-center">
			<div
				class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto"
			></div>
			<p class="mt-3 text-muted-foreground text-sm">กำลังโหลด...</p>
		</div>
	{:else if applications.length === 0}
		<div class="bg-card border border-border rounded-lg p-10 text-center">
			<Users class="w-12 h-12 mx-auto text-muted-foreground mb-3" />
			<p class="text-muted-foreground">ไม่พบใบสมัคร</p>
		</div>
	{:else}
		<div class="bg-card border border-border rounded-lg overflow-hidden">
			<!-- Header -->
			<div
				class="bg-muted/50 px-4 py-2.5 border-b border-border text-xs font-medium text-muted-foreground grid grid-cols-12 gap-3"
			>
				<div class="col-span-1">เลขที่</div>
				<div class="col-span-3">ชื่อ-สกุล</div>
				<div class="col-span-2">เลขบัตร</div>
				<div class="col-span-2">สาย</div>
				<div class="col-span-1">สถานะ</div>
				<div class="col-span-3 text-right">จัดการ</div>
			</div>

			<div class="divide-y divide-border">
				{#each applications as app (app.id)}
					<div
						class="px-4 py-3 hover:bg-accent/30 transition-colors grid grid-cols-12 gap-3 items-center"
					>
						<div class="col-span-1">
							<span class="font-mono text-xs">{app.applicationNumber ?? '-'}</span>
						</div>
						<div class="col-span-3">
							<p class="font-medium text-sm text-foreground">{app.fullName}</p>
							<p class="text-xs text-muted-foreground">{app.phone ?? ''}</p>
						</div>
						<div class="col-span-2">
							<span class="font-mono text-xs text-muted-foreground">{app.nationalId}</span>
						</div>
						<div class="col-span-2">
							<span class="text-xs">{app.trackName ?? '-'}</span>
						</div>
						<div class="col-span-1">
							<span
								class="text-xs px-1.5 py-0.5 rounded-full {applicationStatusColor[app.status] ??
									'bg-gray-100 text-gray-700'}"
							>
								{applicationStatusLabel[app.status] ?? app.status}
							</span>
						</div>
						<div class="col-span-3 flex justify-end gap-1">
							<Button
								href="/staff/academic/admission/{id}/applications/{app.id}"
								variant="ghost"
								size="sm"
							>
								<Eye class="w-3.5 h-3.5" />
							</Button>
							{#if app.status === 'submitted'}
								<Button
									variant="ghost"
									size="sm"
									onclick={() => handleVerify(app)}
									class="text-green-600 hover:text-green-700"
								>
									<Check class="w-3.5 h-3.5" />
								</Button>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => {
										rejectingApp = app;
										showRejectDialog = true;
									}}
									class="text-destructive hover:text-destructive"
								>
									<X class="w-3.5 h-3.5" />
								</Button>
							{/if}
						</div>
					</div>
				{/each}
			</div>

			<div class="px-4 py-3 border-t border-border bg-muted/30">
				<p class="text-xs text-muted-foreground">แสดง {applications.length} รายการ</p>
			</div>
		</div>
	{/if}
</div>

<!-- Reject Dialog -->
<Dialog bind:open={showRejectDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ปฏิเสธใบสมัคร</DialogTitle>
			<DialogDescription>
				ปฏิเสธใบสมัครของ <strong>{rejectingApp?.fullName}</strong>
			</DialogDescription>
		</DialogHeader>
		<div class="space-y-2 py-2">
			<label class="text-sm font-medium">เหตุผล *</label>
			<textarea
				bind:value={rejectReason}
				rows="3"
				class="w-full px-3 py-2 text-sm rounded-md border border-border bg-background resize-none"
				placeholder="ระบุเหตุผลที่ปฏิเสธ..."
			></textarea>
		</div>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showRejectDialog = false)}>ยกเลิก</Button>
			<Button
				variant="destructive"
				onclick={handleRejectConfirm}
				disabled={rejecting || !rejectReason.trim()}
			>
				{rejecting ? 'กำลังดำเนินการ...' : 'ปฏิเสธ'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
