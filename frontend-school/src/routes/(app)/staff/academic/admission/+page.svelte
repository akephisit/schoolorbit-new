<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listRounds,
		type AdmissionRound,
		roundStatusLabel,
		roundStatusColor,
		updateRoundStatus,
		deleteRound
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import {
		ClipboardList,
		Plus,
		Eye,
		Trash2,
		ToggleRight,
		Users,
		Calendar,
		Loader2
	} from 'lucide-svelte';

	let { data } = $props();

	let rounds: AdmissionRound[] = $state([]);
	let loading = $state(true);
	let showDeleteDialog = $state(false);
	let roundToDelete: AdmissionRound | null = $state(null);
	let deleting = $state(false);

	async function load() {
		try {
			loading = true;
			rounds = await listRounds();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	async function toggleOpen(round: AdmissionRound) {
		const next = round.status === 'open' ? 'draft' : 'open';
		try {
			await updateRoundStatus(round.id, next);
			toast.success(`เปลี่ยนสถานะเป็น "${roundStatusLabel[next]}" แล้ว`);
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		}
	}

	async function confirmDelete() {
		if (!roundToDelete) return;
		deleting = true;
		try {
			await deleteRound(roundToDelete.id);
			toast.success('ลบรอบรับสมัครแล้ว');
			showDeleteDialog = false;
			roundToDelete = null;
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}

	function formatDate(d?: string) {
		if (!d) return '-';
		return new Date(d).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	// Map status → Badge variant
	const statusVariant: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
		draft: 'secondary',
		open: 'default',
		exam: 'default',
		scoring: 'default',
		announced: 'default',
		enrolling: 'default',
		closed: 'destructive'
	};

	onMount(load);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<ClipboardList class="w-8 h-8" />
				ระบบรับสมัครนักเรียน
			</h1>
			<p class="text-muted-foreground mt-1">จัดการรอบรับสมัคร สายการเรียน และใบสมัคร</p>
		</div>
		<Button href="/staff/academic/admission/new" class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			สร้างรอบรับสมัครใหม่
		</Button>
	</div>

	<!-- Rounds List -->
	{#if loading}
		<Card.Root>
			<Card.Content class="flex items-center justify-center py-16">
				<div class="flex flex-col items-center gap-3 text-muted-foreground">
					<Loader2 class="w-8 h-8 animate-spin" />
					<p>กำลังโหลด...</p>
				</div>
			</Card.Content>
		</Card.Root>
	{:else if rounds.length === 0}
		<Card.Root>
			<Card.Content class="flex flex-col items-center py-16 gap-3">
				<ClipboardList class="w-16 h-16 text-muted-foreground" />
				<p class="text-lg font-medium text-foreground">ยังไม่มีรอบรับสมัคร</p>
				<p class="text-muted-foreground text-sm">เริ่มต้นด้วยการสร้างรอบรับสมัครแรก</p>
				<Button href="/staff/academic/admission/new" class="mt-2">
					<Plus class="w-4 h-4 mr-2" />
					สร้างรอบรับสมัคร
				</Button>
			</Card.Content>
		</Card.Root>
	{:else}
		<div class="grid gap-4">
			{#each rounds as round (round.id)}
				<Card.Root class="hover:shadow-md transition-shadow">
					<Card.Content class="p-5">
						<div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
							<div class="space-y-2">
								<div class="flex items-center gap-2 flex-wrap">
									<h2 class="text-lg font-semibold text-foreground">{round.name}</h2>
									<Badge
										variant={statusVariant[round.status] ?? 'secondary'}
										class={roundStatusColor[round.status]}
									>
										{roundStatusLabel[round.status] ?? round.status}
									</Badge>
									{#if round.gradeLevelName}
										<Badge variant="outline">{round.gradeLevelName}</Badge>
									{/if}
								</div>
								<div class="flex items-center gap-4 text-sm text-muted-foreground flex-wrap">
									<span class="flex items-center gap-1">
										<Calendar class="w-3.5 h-3.5" />
										รับสมัคร: {formatDate(round.applyStartDate)} – {formatDate(round.applyEndDate)}
									</span>
									{#if round.applicationCount !== undefined}
										<span class="flex items-center gap-1">
											<Users class="w-3.5 h-3.5" />
											{round.applicationCount} ใบสมัคร
										</span>
									{/if}
									{#if round.academicYearName}
										<span>ปีการศึกษา {round.academicYearName}</span>
									{/if}
								</div>
							</div>

							<div class="flex items-center gap-2 flex-wrap">
								<Button
									variant="outline"
									size="sm"
									onclick={() => toggleOpen(round)}
									class="text-xs"
								>
									<ToggleRight class="w-3.5 h-3.5 mr-1" />
									{round.status === 'open' ? 'ปิดรับสมัคร' : 'เปิดรับสมัคร'}
								</Button>
								<Button href="/staff/academic/admission/{round.id}" variant="outline" size="sm">
									<Eye class="w-4 h-4 mr-1" />
									จัดการ
								</Button>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => {
										roundToDelete = round;
										showDeleteDialog = true;
									}}
									class="text-destructive hover:text-destructive"
								>
									<Trash2 class="w-4 h-4" />
								</Button>
							</div>
						</div>
					</Card.Content>
				</Card.Root>
			{/each}
		</div>
	{/if}
</div>

<!-- Delete Confirm Dialog -->
<Dialog.Root bind:open={showDeleteDialog}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>ยืนยันการลบรอบรับสมัคร</Dialog.Title>
			<Dialog.Description>
				ลบ <strong>{roundToDelete?.name}</strong>? รอบที่มีใบสมัครอยู่จะไม่สามารถลบได้
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
				ยกเลิก
			</Button>
			<Button variant="destructive" onclick={confirmDelete} disabled={deleting}>
				{#if deleting}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
				{deleting ? 'กำลังลบ...' : 'ลบรอบ'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
