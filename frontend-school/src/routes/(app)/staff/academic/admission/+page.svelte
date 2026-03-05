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
	import { toast } from 'svelte-sonner';
	import { ClipboardList, Plus, Eye, Trash2, ToggleRight, Users, Calendar } from 'lucide-svelte';
	import {
		Dialog,
		DialogContent,
		DialogHeader,
		DialogTitle,
		DialogDescription,
		DialogFooter
	} from '$lib/components/ui/dialog';

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

	onMount(load);
</script>

<svelte:head>
	<title>ระบบรับสมัครนักเรียน - SchoolOrbit</title>
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
		<Button href="academic/admission/new" class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			สร้างรอบรับสมัครใหม่
		</Button>
	</div>

	<!-- Rounds List -->
	{#if loading}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<div
				class="inline-block w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</div>
	{:else if rounds.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<ClipboardList class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
			<p class="text-lg font-medium text-foreground">ยังไม่มีรอบรับสมัคร</p>
			<p class="text-muted-foreground mt-2">เริ่มต้นด้วยการสร้างรอบรับสมัครแรก</p>
			<Button href="academic/admission/new" class="mt-4">
				<Plus class="w-4 h-4 mr-2" />
				สร้างรอบรับสมัคร
			</Button>
		</div>
	{:else}
		<div class="grid gap-4">
			{#each rounds as round (round.id)}
				<div class="bg-card border border-border rounded-lg p-5 hover:shadow-md transition-shadow">
					<div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
						<div class="space-y-1.5">
							<div class="flex items-center gap-2 flex-wrap">
								<h2 class="text-lg font-semibold text-foreground">{round.name}</h2>
								<span
									class="text-xs px-2 py-0.5 rounded-full font-medium {roundStatusColor[
										round.status
									] || 'bg-gray-100 text-gray-700'}"
								>
									{roundStatusLabel[round.status] || round.status}
								</span>
								{#if round.gradeLevelName}
									<span class="text-xs px-2 py-0.5 bg-blue-50 text-blue-700 rounded-full">
										{round.gradeLevelName}
									</span>
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
							<Button variant="outline" size="sm" onclick={() => toggleOpen(round)} class="text-xs">
								<ToggleRight class="w-3.5 h-3.5 mr-1" />
								{round.status === 'open' ? 'ปิดรับสมัคร' : 'เปิดรับสมัคร'}
							</Button>
							<Button href="academic/admission/{round.id}" variant="outline" size="sm">
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
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Delete Confirm Dialog -->
<Dialog bind:open={showDeleteDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการลบรอบรับสมัคร</DialogTitle>
			<DialogDescription>
				ลบ <strong>{roundToDelete?.name}</strong>? รอบที่มีใบสมัครอยู่จะไม่สามารถลบได้
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}
				>ยกเลิก</Button
			>
			<Button variant="destructive" onclick={confirmDelete} disabled={deleting}>
				{deleting ? 'กำลังลบ...' : 'ลบรอบ'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
