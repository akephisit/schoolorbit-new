<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getRound, listEnrollmentPending, completeEnrollment } from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, ClipboardCheck, Check, UserCheck } from 'lucide-svelte';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogDescription,
		DialogFooter
	} from '$lib/components/ui/dialog';

	interface EnrollRow {
		id: string;
		applicationNumber?: string;
		nationalId: string;
		fullName: string;
		trackName?: string;
		roomName?: string;
		status: string;
		studentConfirmed?: boolean;
		preSubmitted: boolean;
	}

	let id = $derived($page.params.id);
	let round: Awaited<ReturnType<typeof getRound>> | null = $state(null);
	let list: EnrollRow[] = $state([]);
	let loading = $state(true);

	// Enroll dialog
	let showEnrollDialog = $state(false);
	let enrollingApp: EnrollRow | null = $state(null);
	let studentCode = $state('');
	let enrolling = $state(false);

	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, l] = await Promise.all([getRound(id), listEnrollmentPending(id)]);
			round = r;
			list = (l as EnrollRow[]) ?? [];
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function handleEnroll() {
		if (!enrollingApp) return;
		enrolling = true;
		try {
			const res = (await completeEnrollment(enrollingApp.id, studentCode || undefined)) as {
				username?: string;
				studentCode?: string;
			};
			toast.success(`มอบตัวสำเร็จ! Username: ${res?.username}`);
			showEnrollDialog = false;
			enrollingApp = null;
			studentCode = '';
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'มอบตัวไม่สำเร็จ');
		} finally {
			enrolling = false;
		}
	}

	onMount(load);
</script>

<svelte:head>
	<title>รับมอบตัว - SchoolOrbit</title>
</svelte:head>

<div class="space-y-5">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold flex items-center gap-2">
			<ClipboardCheck class="w-6 h-6" /> รับมอบตัว
		</h1>
	</div>

	{#if round}
		<p class="text-sm text-muted-foreground">{round.name}</p>
	{/if}

	<!-- Stats -->
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
		<div class="bg-card border border-border rounded-lg p-4 text-center">
			<p class="text-2xl font-bold text-foreground">{list.length}</p>
			<p class="text-xs text-muted-foreground mt-1">ได้รับคัดเลือกทั้งหมด</p>
		</div>
		<div class="bg-green-50 border border-green-200 rounded-lg p-4 text-center">
			<p class="text-2xl font-bold text-green-700">
				{list.filter((a) => a.studentConfirmed).length}
			</p>
			<p class="text-xs text-green-600 mt-1">ยืนยันแล้ว</p>
		</div>
		<div class="bg-blue-50 border border-blue-200 rounded-lg p-4 text-center">
			<p class="text-2xl font-bold text-blue-700">{list.filter((a) => a.preSubmitted).length}</p>
			<p class="text-xs text-blue-600 mt-1">กรอกฟอร์มล่วงหน้า</p>
		</div>
		<div class="bg-purple-50 border border-purple-200 rounded-lg p-4 text-center">
			<p class="text-2xl font-bold text-purple-700">
				{list.filter((a) => a.status === 'enrolled').length}
			</p>
			<p class="text-xs text-purple-600 mt-1">มอบตัวแล้ว</p>
		</div>
	</div>

	{#if loading}
		<div class="bg-card border border-border rounded-lg p-10 text-center">
			<div
				class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto"
			></div>
		</div>
	{:else if list.length === 0}
		<div class="bg-card border border-border rounded-lg p-10 text-center text-muted-foreground">
			<UserCheck class="w-12 h-12 mx-auto mb-3 opacity-40" />
			<p>ยังไม่มีรายชื่อที่รอมอบตัว</p>
			<p class="text-xs mt-1">ต้องผ่านขั้นตอนจัดห้องก่อน</p>
		</div>
	{:else}
		<div class="bg-card border border-border rounded-lg overflow-hidden">
			<div
				class="bg-muted/50 px-4 py-2.5 border-b border-border text-xs font-medium text-muted-foreground grid grid-cols-12 gap-3"
			>
				<div class="col-span-1">เลขที่</div>
				<div class="col-span-3">ชื่อ</div>
				<div class="col-span-2">สาย</div>
				<div class="col-span-2">ห้อง</div>
				<div class="col-span-2">สถานะ</div>
				<div class="col-span-2 text-right">จัดการ</div>
			</div>
			<div class="divide-y divide-border">
				{#each list as app (app.id)}
					<div
						class="px-4 py-3 hover:bg-accent/20 transition-colors grid grid-cols-12 gap-3 items-center {app.status ===
						'enrolled'
							? 'opacity-60'
							: ''}"
					>
						<div class="col-span-1 font-mono text-xs">{app.applicationNumber ?? '-'}</div>
						<div class="col-span-3">
							<p class="font-medium text-sm">{app.fullName}</p>
							<p class="text-xs text-muted-foreground">{app.nationalId}</p>
						</div>
						<div class="col-span-2 text-xs">{app.trackName ?? '-'}</div>
						<div class="col-span-2 text-xs">{app.roomName ?? '-'}</div>
						<div class="col-span-2">
							<div class="flex flex-col gap-0.5">
								{#if app.status === 'enrolled'}
									<span
										class="text-xs px-1.5 py-0.5 bg-purple-100 text-purple-700 rounded-full w-fit"
										>มอบตัวแล้ว</span
									>
								{:else}
									{#if app.studentConfirmed}
										<span
											class="text-xs px-1.5 py-0.5 bg-green-100 text-green-700 rounded-full w-fit"
											>ยืนยันแล้ว</span
										>
									{:else}
										<span
											class="text-xs px-1.5 py-0.5 bg-yellow-100 text-yellow-700 rounded-full w-fit"
											>ยังไม่ยืนยัน</span
										>
									{/if}
									{#if app.preSubmitted}
										<span class="text-xs px-1.5 py-0.5 bg-blue-100 text-blue-700 rounded-full w-fit"
											>กรอกฟอร์มแล้ว</span
										>
									{/if}
								{/if}
							</div>
						</div>
						<div class="col-span-2 flex justify-end">
							{#if app.status !== 'enrolled'}
								<Button
									size="sm"
									onclick={() => {
										enrollingApp = app;
										showEnrollDialog = true;
									}}
									class="gap-1 h-7 text-xs"
								>
									<Check class="w-3 h-3" /> รับมอบตัว
								</Button>
							{:else}
								<span class="text-xs text-green-600 flex items-center gap-1">
									<Check class="w-3 h-3" /> เสร็จสิ้น
								</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>

<!-- Enroll Dialog -->
<Dialog bind:open={showEnrollDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>รับมอบตัว — สร้าง Account</DialogTitle>
			<DialogDescription>
				{enrollingApp?.fullName} ({enrollingApp?.nationalId})
				<br />ห้อง: {enrollingApp?.roomName ?? '-'}
			</DialogDescription>
		</DialogHeader>
		<div class="space-y-3 py-2">
			<div class="space-y-1.5">
				<label class="text-sm font-medium">รหัสนักเรียน (ไม่บังคับ)</label>
				<Input bind:value={studentCode} placeholder="ระบบจะสร้างให้อัตโนมัติถ้าว่าง" />
			</div>
			<div class="text-xs text-muted-foreground bg-muted/50 rounded p-2">
				<p>• Username สร้างโดยอัตโนมัติ: <code>s{enrollingApp?.nationalId}</code></p>
				<p>• Password เริ่มต้น: เลขบัตรประชาชน (แนะนำให้เปลี่ยนทันที)</p>
			</div>
		</div>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showEnrollDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleEnroll} disabled={enrolling}>
				{enrolling ? 'กำลังสร้าง Account...' : 'ยืนยันมอบตัว'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
