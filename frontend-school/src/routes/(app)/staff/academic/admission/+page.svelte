<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		listRounds,
		type AdmissionRound,
		roundStatusLabel,
		roundStatusColor,
		updateRoundStatus,
		deleteRound
	} from '#lib/api/admission.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import {
		LoadingButton,
		PageSkeleton,
		PageState,
		RegionUpdatingState
	} from '#lib/components/app-state/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { toast } from 'svelte-sonner';
	import { Plus, Eye, Trash2, ToggleRight, Users, Calendar, RefreshCw } from '@lucide/svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const canReadAdmission = $derived($can.has(PERMISSIONS.ADMISSION_READ_ALL));
	const canManageAdmission = $derived($can.has(PERMISSIONS.ADMISSION_MANAGE_ALL));
	const academicYearId = $derived(data.academicYearId);
	const request = new LatestRequest();

	let rounds: AdmissionRound[] = $state([]);
	let loading = $state(false);
	let loaded = $state(false);
	let error = $state('');
	let showDeleteDialog = $state(false);
	let roundToDelete: AdmissionRound | null = $state(null);
	let deleting = $state(false);

	async function load() {
		if (!canReadAdmission || !academicYearId) return;
		const selectedYear = academicYearId;
		const { revision, signal } = request.begin();
		loading = true;
		error = '';
		try {
			const next = await listRounds(selectedYear, { signal });
			if (!request.isCurrent(revision)) return;
			rounds = next;
			loaded = true;
		} catch (e) {
			if (!isAbortError(e) && request.isCurrent(revision))
				error = e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	async function toggleOpen(round: AdmissionRound) {
		if (!canManageAdmission) {
			toast.error('ไม่มีสิทธิ์จัดการรอบรับสมัคร');
			return;
		}
		const sourceYear = academicYearId;
		const next = round.status === 'open' ? 'draft' : 'open';
		try {
			await updateRoundStatus(round.id, next);
			if (academicYearId !== sourceYear) return;
			request.abort();
			loading = false;
			error = '';
			rounds = rounds.map((item) => (item.id === round.id ? { ...item, status: next } : item));
			toast.success(`เปลี่ยนสถานะเป็น "${roundStatusLabel[next]}" แล้ว`);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		}
	}

	async function confirmDelete() {
		if (!canManageAdmission) {
			toast.error('ไม่มีสิทธิ์ลบรอบรับสมัคร');
			return;
		}
		if (!roundToDelete) return;
		const sourceYear = academicYearId;
		const targetId = roundToDelete.id;
		deleting = true;
		try {
			await deleteRound(targetId);
			if (academicYearId !== sourceYear) return;
			request.abort();
			loading = false;
			error = '';
			rounds = rounds.filter((item) => item.id !== targetId);
			toast.success('ลบรอบรับสมัครแล้ว');
			showDeleteDialog = false;
			roundToDelete = null;
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

	$effect.pre(() => {
		const routeRounds = data.rounds;
		const { revision } = request.begin();
		untrack(() => {
			rounds = [];
			loaded = false;
			loading = Boolean(routeRounds);
			error = '';
			showDeleteDialog = false;
			roundToDelete = null;
		});
		if (routeRounds)
			void routeRounds.then((result) => {
				if (!request.isCurrent(revision)) return;
				untrack(() => {
					if (result.ok) {
						rounds = result.data;
						loaded = true;
					} else error = result.error;
					loading = false;
				});
			});
		return () => request.abort();
	});
</script>

<PageShell title="ระบบรับสมัครนักเรียน" description="จัดการรอบรับสมัคร สายการเรียน และใบสมัคร">
	{#snippet actions()}
		<Button
			variant="outline"
			size="icon"
			onclick={() => void load()}
			disabled={loading || !academicYearId}
			aria-label="โหลดรอบใหม่"
		>
			<RefreshCw class="size-4" />
		</Button>
		{#if canManageAdmission}
			<Button
				href={`/staff/academic/admission/new?academicYearId=${encodeURIComponent(academicYearId ?? '')}`}
				class="flex items-center gap-2"
			>
				<Plus class="w-4 h-4" />
				สร้างรอบรับสมัครใหม่
			</Button>
		{/if}
	{/snippet}

	<!-- Rounds List -->
	{#if !canReadAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรอบรับสมัคร"
			description="บัญชีนี้เข้า module รับสมัครได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลรอบรับสมัคร"
		/>
	{:else if !academicYearId}
		<PageState title="เลือกปีการศึกษาก่อน" description="ใช้ตัวเลือกปีการศึกษาบนแถบด้านบน" />
	{:else if loading && !loaded}
		<PageSkeleton variant="cards" rows={3} />
	{:else if error && !loaded}
		<PageState
			variant="error"
			title="โหลดรอบรับสมัครไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => void load()}
		/>
	{:else}
		<div data-testid="admission-round-list-region" aria-busy={loading}>
			{#if loading}<RegionUpdatingState class="static" label="กำลังอัปเดตรอบรับสมัคร..." />{/if}
			{#if error}<p role="alert" class="text-sm text-destructive">
					{error}
					<Button variant="outline" size="sm" onclick={() => void load()}>ลองใหม่</Button>
				</p>{/if}
			{#if rounds.length === 0}
				<PageState
					title="ยังไม่มีรอบรับสมัคร"
					description="เริ่มต้นด้วยการสร้างรอบรับสมัครแรก"
					actionLabel={canManageAdmission ? 'สร้างรอบรับสมัคร' : undefined}
					href={canManageAdmission
						? `/staff/academic/admission/new?academicYearId=${encodeURIComponent(academicYearId)}`
						: undefined}
				/>
			{:else}
				<div class="grid gap-4">
					{#each rounds as round (round.id)}
						<Card.Root class="gap-0 py-0 transition-shadow hover:shadow-md">
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
												รับสมัคร: {formatDate(round.applyStartDate)} – {formatDate(
													round.applyEndDate
												)}
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
										{#if canManageAdmission}
											<Button
												variant="outline"
												size="sm"
												onclick={() => toggleOpen(round)}
												class="text-xs"
											>
												<ToggleRight class="w-3.5 h-3.5 mr-1" />
												{round.status === 'open' ? 'ปิดรับสมัคร' : 'เปิดรับสมัคร'}
											</Button>
										{/if}
										<Button href="/staff/academic/admission/{round.id}" variant="outline" size="sm">
											<Eye class="w-4 h-4 mr-1" />
											จัดการ
										</Button>
										{#if canManageAdmission}
											<Button
												variant="ghost"
												size="sm"
												aria-label={`ลบรอบ ${round.name}`}
												onclick={() => {
													roundToDelete = round;
													showDeleteDialog = true;
												}}
												class="text-destructive hover:text-destructive"
											>
												<Trash2 class="w-4 h-4" />
											</Button>
										{/if}
									</div>
								</div>
							</Card.Content>
						</Card.Root>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</PageShell>

<!-- Delete Confirm Dialog -->
{#if canManageAdmission}
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบรอบรับสมัคร</Dialog.Title>
				<Dialog.Description>
					ลบ <strong>{roundToDelete?.name}</strong> พร้อมใบสมัคร
					{#if roundToDelete?.applicationCount != null}{roundToDelete.applicationCount} รายการ{:else}ทั้งหมด{/if}
					และเอกสารที่เกี่ยวข้องอย่างถาวร? การดำเนินการนี้ย้อนกลับไม่ได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
					ยกเลิก
				</Button>
				<LoadingButton
					variant="destructive"
					onclick={confirmDelete}
					loading={deleting}
					loadingLabel="กำลังลบ..."
				>
					ลบรอบและใบสมัครทั้งหมด
				</LoadingButton>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}
