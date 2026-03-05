<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getRound, listEnrollmentPending, completeEnrollment } from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, ClipboardCheck, Check, UserCheck, Loader2 } from 'lucide-svelte';

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

	let { data } = $props();
	let id = $derived($page.params.id);

	let round: Awaited<ReturnType<typeof getRound>> | null = $state(null);
	let list: EnrollRow[] = $state([]);
	let loading = $state(true);

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
	<title>{data.title} - SchoolOrbit</title>
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
		<Card.Root>
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold">{list.length}</p>
				<p class="text-xs text-muted-foreground mt-1">ได้รับคัดเลือกทั้งหมด</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-green-200 bg-green-50 dark:bg-green-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-green-700">
					{list.filter((a) => a.studentConfirmed).length}
				</p>
				<p class="text-xs text-green-600 mt-1">ยืนยันแล้ว</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-blue-200 bg-blue-50 dark:bg-blue-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-blue-700">{list.filter((a) => a.preSubmitted).length}</p>
				<p class="text-xs text-blue-600 mt-1">กรอกฟอร์มล่วงหน้า</p>
			</Card.Content>
		</Card.Root>
		<Card.Root class="border-purple-200 bg-purple-50 dark:bg-purple-950/20">
			<Card.Content class="pt-5 pb-5 text-center">
				<p class="text-3xl font-bold text-purple-700">
					{list.filter((a) => a.status === 'enrolled').length}
				</p>
				<p class="text-xs text-purple-600 mt-1">มอบตัวแล้ว</p>
			</Card.Content>
		</Card.Root>
	</div>

	{#if loading}
		<Card.Root>
			<Card.Content class="flex justify-center py-16">
				<Loader2 class="w-8 h-8 animate-spin text-primary" />
			</Card.Content>
		</Card.Root>
	{:else if list.length === 0}
		<Card.Root>
			<Card.Content class="flex flex-col items-center py-16 gap-3 text-muted-foreground">
				<UserCheck class="w-12 h-12 opacity-40" />
				<p>ยังไม่มีรายชื่อที่รอมอบตัว</p>
				<p class="text-xs">ต้องผ่านขั้นตอนจัดห้องก่อน</p>
			</Card.Content>
		</Card.Root>
	{:else}
		<Card.Root>
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="w-24">เลขที่</Table.Head>
						<Table.Head>ชื่อ</Table.Head>
						<Table.Head>สาย</Table.Head>
						<Table.Head>ห้อง</Table.Head>
						<Table.Head>สถานะ</Table.Head>
						<Table.Head class="text-right">จัดการ</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each list as app (app.id)}
						<Table.Row class={app.status === 'enrolled' ? 'opacity-60' : ''}>
							<Table.Cell class="font-mono text-xs">{app.applicationNumber ?? '-'}</Table.Cell>
							<Table.Cell>
								<p class="font-medium text-sm">{app.fullName}</p>
								<p class="text-xs text-muted-foreground">{app.nationalId}</p>
							</Table.Cell>
							<Table.Cell class="text-sm">{app.trackName ?? '-'}</Table.Cell>
							<Table.Cell class="text-sm">{app.roomName ?? '-'}</Table.Cell>
							<Table.Cell>
								<div class="flex flex-col gap-1">
									{#if app.status === 'enrolled'}
										<Badge variant="default" class="bg-purple-600 w-fit">มอบตัวแล้ว</Badge>
									{:else}
										<Badge variant={app.studentConfirmed ? 'default' : 'secondary'} class="w-fit">
											{app.studentConfirmed ? 'ยืนยันแล้ว' : 'ยังไม่ยืนยัน'}
										</Badge>
										{#if app.preSubmitted}
											<Badge variant="outline" class="w-fit text-xs">กรอกฟอร์มแล้ว</Badge>
										{/if}
									{/if}
								</div>
							</Table.Cell>
							<Table.Cell class="text-right">
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
									<span class="text-xs text-green-600 flex items-center justify-end gap-1">
										<Check class="w-3 h-3" /> เสร็จสิ้น
									</span>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</Card.Root>
	{/if}
</div>

<!-- Enroll Dialog -->
<Dialog.Root bind:open={showEnrollDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>รับมอบตัว — สร้าง Account</Dialog.Title>
			<Dialog.Description>
				{enrollingApp?.fullName} ({enrollingApp?.nationalId})
				<br />ห้อง: {enrollingApp?.roomName ?? '-'}
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="space-y-1.5">
				<Label for="student-code">รหัสนักเรียน (ไม่บังคับ)</Label>
				<Input
					id="student-code"
					bind:value={studentCode}
					placeholder="ระบบจะสร้างให้อัตโนมัติถ้าว่าง"
				/>
			</div>
			<div class="text-xs text-muted-foreground bg-muted rounded p-2 space-y-0.5">
				<p>• Username: <code>s{enrollingApp?.nationalId}</code></p>
				<p>• Password เริ่มต้น: เลขบัตรประชาชน</p>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showEnrollDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleEnroll} disabled={enrolling}>
				{#if enrolling}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
				{enrolling ? 'กำลังสร้าง Account...' : 'ยืนยันมอบตัว'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
