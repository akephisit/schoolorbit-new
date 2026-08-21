<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import {
		getCertificateCampaignPurgeImpact,
		getCertificateCampaignPurgeStatus,
		retryCertificateCampaignPurge,
		startCertificateCampaignPurge,
		type CertificateCampaignPurgeImpact,
		type CertificateCampaignPurgePhase,
		type CertificateCampaignPurgeStatus
	} from '$lib/api/certificates';
	import { ApiClientError } from '$lib/api/client';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Progress } from '$lib/components/ui/progress';
	import {
		AlertTriangle,
		CheckCircle2,
		FileX2,
		LoaderCircle,
		RefreshCw,
		ShieldAlert,
		Trash2
	} from 'lucide-svelte';

	type ViewPhase =
		| 'loading_impact'
		| 'confirm'
		| 'starting'
		| 'load_error'
		| CertificateCampaignPurgePhase;

	let {
		open,
		campaignId,
		campaignName,
		initiallyPurging = false,
		onopenchange,
		oncompleted
	}: {
		open: boolean;
		campaignId: string;
		campaignName: string;
		initiallyPurging?: boolean;
		onopenchange: (open: boolean) => void;
		oncompleted: () => void;
	} = $props();

	let impact = $state.raw<CertificateCampaignPurgeImpact | null>(null);
	let status = $state.raw<CertificateCampaignPurgeStatus | null>(null);
	let viewPhase = $state<ViewPhase>('loading_impact');
	let confirmationName = $state('');
	let errorMessage = $state('');
	let noticeMessage = $state('');
	let activeController: AbortController | null = null;
	let pollTimer: ReturnType<typeof setTimeout> | null = null;
	let observedPurge = false;
	let completionEmitted = false;

	const starting = $derived(viewPhase === 'starting');
	const running = $derived(viewPhase === 'deleting_files' || viewPhase === 'finalizing');
	const canConfirm = $derived(
		viewPhase === 'confirm' && impact !== null && confirmationName === impact.campaignName
	);
	const progressMax = $derived(Math.max(status?.fileCount ?? 0, 1));
	const progressValue = $derived(Math.min(status?.deletedFileCount ?? 0, progressMax));

	const impactItems = $derived(
		impact
			? ([
					['แม่แบบ', impact.counts.templateCount],
					['รายชื่อผู้รับ', impact.counts.candidateCount],
					['คำขอออก', impact.counts.requestCount],
					['คำขอที่ยังไม่จบ', impact.counts.openRequestCount],
					['เกียรติบัตรที่ออกแล้ว', impact.counts.issuedCertificateCount],
					['เกียรติบัตรที่เพิกถอน', impact.counts.revokedCertificateCount],
					['ไฟล์', impact.counts.fileCount]
				] satisfies Array<[string, number]>)
			: []
	);

	function nextController(): AbortController {
		activeController?.abort();
		const controller = new AbortController();
		activeController = controller;
		return controller;
	}

	function clearPolling(): void {
		if (pollTimer !== null) {
			clearTimeout(pollTimer);
			pollTimer = null;
		}
	}

	function cancelAsync(): void {
		clearPolling();
		activeController?.abort();
		activeController = null;
	}

	function emitCompletion(): void {
		if (completionEmitted) return;
		completionEmitted = true;
		viewPhase = 'completed';
		clearPolling();
		oncompleted();
	}

	function schedulePoll(): void {
		clearPolling();
		pollTimer = setTimeout(() => {
			pollTimer = null;
			void refreshStatus();
		}, 1_500);
	}

	function applyStatus(nextStatus: CertificateCampaignPurgeStatus): void {
		observedPurge = true;
		status = nextStatus;
		viewPhase = nextStatus.phase;
		errorMessage = '';
		if (nextStatus.phase === 'completed') {
			emitCompletion();
		} else if (nextStatus.phase === 'deleting_files' || nextStatus.phase === 'finalizing') {
			schedulePoll();
		}
	}

	async function loadImpact(staleNotice = ''): Promise<void> {
		clearPolling();
		viewPhase = 'loading_impact';
		errorMessage = '';
		noticeMessage = staleNotice;
		const controller = nextController();
		try {
			impact = await getCertificateCampaignPurgeImpact(campaignId, {
				signal: controller.signal
			});
			if (controller.signal.aborted) return;
			confirmationName = '';
			viewPhase = 'confirm';
		} catch (error) {
			if (controller.signal.aborted) return;
			if (error instanceof ApiClientError && error.status === 409) {
				observedPurge = true;
				viewPhase = 'deleting_files';
				await refreshStatus();
				return;
			}
			viewPhase = 'load_error';
			errorMessage = error instanceof Error ? error.message : 'ไม่สามารถตรวจสอบข้อมูลที่จะลบได้';
		}
	}

	async function refreshStatus(): Promise<void> {
		clearPolling();
		const controller = nextController();
		try {
			const nextStatus = await getCertificateCampaignPurgeStatus(campaignId, {
				signal: controller.signal
			});
			if (controller.signal.aborted) return;
			applyStatus(nextStatus);
		} catch (error) {
			if (controller.signal.aborted) return;
			if (error instanceof ApiClientError && error.status === 404 && observedPurge) {
				emitCompletion();
				return;
			}
			viewPhase = 'load_error';
			errorMessage = error instanceof Error ? error.message : 'ไม่สามารถโหลดสถานะการลบได้';
		}
	}

	async function startPurge(): Promise<void> {
		if (!canConfirm || !impact) return;
		viewPhase = 'starting';
		errorMessage = '';
		noticeMessage = '';
		const controller = nextController();
		try {
			const nextStatus = await startCertificateCampaignPurge(
				campaignId,
				{
					confirmationName,
					expectedUpdatedAt: impact.updatedAt,
					expectedImpact: impact.counts
				},
				{ signal: controller.signal }
			);
			if (controller.signal.aborted) return;
			applyStatus(nextStatus);
		} catch (error) {
			if (controller.signal.aborted) return;
			if (error instanceof ApiClientError && error.status === 409) {
				await loadImpact('ข้อมูลกิจกรรมเปลี่ยนแล้ว โปรดตรวจจำนวนทั้งหมดอีกครั้ง');
				return;
			}
			viewPhase = 'confirm';
			errorMessage = error instanceof Error ? error.message : 'ไม่สามารถเริ่มลบกิจกรรมได้';
		}
	}

	async function retryPurge(): Promise<void> {
		viewPhase = 'starting';
		errorMessage = '';
		const controller = nextController();
		try {
			const nextStatus = await retryCertificateCampaignPurge(campaignId, {
				signal: controller.signal
			});
			if (controller.signal.aborted) return;
			applyStatus(nextStatus);
		} catch (error) {
			if (controller.signal.aborted) return;
			if (error instanceof ApiClientError && error.status === 404 && observedPurge) {
				emitCompletion();
				return;
			}
			viewPhase = 'failed';
			errorMessage = error instanceof Error ? error.message : 'ไม่สามารถลองลบกิจกรรมต่อได้';
		}
	}

	function changeOpen(nextOpen: boolean): void {
		if (!nextOpen && starting) return;
		if (!nextOpen) cancelAsync();
		onopenchange(nextOpen);
	}

	function formatCount(value: number): string {
		return value.toLocaleString('th-TH');
	}

	function formatBytes(value: number): string {
		if (value < 1_024) return `${formatCount(value)} ไบต์`;
		const units = ['KB', 'MB', 'GB', 'TB'];
		let size = value / 1_024;
		let unitIndex = 0;
		while (size >= 1_024 && unitIndex < units.length - 1) {
			size /= 1_024;
			unitIndex += 1;
		}
		return `${size.toLocaleString('th-TH', { maximumFractionDigits: 1 })} ${units[unitIndex]}`;
	}

	onMount(() => {
		if (initiallyPurging) {
			observedPurge = true;
			viewPhase = 'deleting_files';
			void refreshStatus();
		} else {
			void loadImpact();
		}
	});

	onDestroy(cancelAsync);
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-2xl" showCloseButton={!starting}>
		<Dialog.Header>
			<div
				class="mb-2 flex size-11 items-center justify-center rounded-full bg-destructive/10 text-destructive"
			>
				<Trash2 class="size-5" />
			</div>
			<Dialog.Title>ลบกิจกรรมถาวร</Dialog.Title>
			<Dialog.Description>
				{campaignName} · ลบข้อมูลและไฟล์ทั้งหมดของกิจกรรมนี้
			</Dialog.Description>
		</Dialog.Header>

		{#if viewPhase === 'loading_impact'}
			<div
				class="flex min-h-52 flex-col items-center justify-center gap-3 text-center"
				aria-live="polite"
			>
				<LoaderCircle class="size-7 animate-spin text-destructive" />
				<div>
					<p class="font-medium">กำลังตรวจข้อมูลที่จะลบ</p>
					<p class="mt-1 text-sm text-muted-foreground">ระบบกำลังนับข้อมูลและไฟล์ล่าสุด</p>
				</div>
			</div>
		{:else if viewPhase === 'confirm' && impact}
			<div class="space-y-5">
				<div class="overflow-hidden rounded-xl border border-destructive/20">
					<div class="border-l-4 border-destructive bg-destructive/5 px-4 py-3">
						<p class="text-sm font-semibold text-destructive">รายการที่จะถูกลบออกจากระบบ</p>
						<p class="mt-1 text-xs text-muted-foreground">ข้อมูลนี้เป็นภาพรวมล่าสุดก่อนยืนยัน</p>
					</div>
					<dl class="grid grid-cols-2 sm:grid-cols-4">
						{#each impactItems as [label, value] (label)}
							<div class="border-t p-3 first:border-t-0 sm:[&:nth-child(-n+4)]:border-t-0">
								<dt class="text-xs text-muted-foreground">{label}</dt>
								<dd class="mt-1 text-lg font-semibold tabular-nums">{formatCount(value)}</dd>
							</div>
						{/each}
						<div class="border-t p-3">
							<dt class="text-xs text-muted-foreground">พื้นที่ไฟล์</dt>
							<dd class="mt-1 text-lg font-semibold tabular-nums">
								{formatBytes(impact.counts.totalFileBytes)}
							</dd>
						</div>
					</dl>
				</div>

				<div class="space-y-2 rounded-xl border border-red-200 bg-red-50 p-4 text-red-950">
					<div class="flex gap-3">
						<ShieldAlert class="mt-0.5 size-5 shrink-0" />
						<div>
							<p class="font-medium">การลบนี้ย้อนกลับไม่ได้</p>
							<ul class="mt-2 list-disc space-y-1 pl-5 text-sm text-red-800">
								<li>หน้าสาธารณะจะตรวจสอบเกียรติบัตรไม่ได้ทันทีเมื่อเริ่มลบ</li>
								<li>ไฟล์จริงและข้อมูลไฟล์ในระบบจะถูกลบถาวร</li>
								<li>เลขที่ออกแล้วจะไม่นำกลับมาใช้ซ้ำ แม้กิจกรรมถูกลบ</li>
							</ul>
						</div>
					</div>
				</div>

				{#if noticeMessage}
					<div
						class="flex gap-2 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900"
					>
						<AlertTriangle class="mt-0.5 size-4 shrink-0" />
						<span>{noticeMessage}</span>
					</div>
				{/if}

				<label class="block space-y-2">
					<span class="text-sm font-medium">พิมพ์ชื่อกิจกรรมเพื่อยืนยัน</span>
					<Input
						bind:value={confirmationName}
						autocomplete="off"
						spellcheck="false"
						placeholder={impact.campaignName}
					/>
					<span class="text-xs text-muted-foreground">
						ต้องตรงกับ “{impact.campaignName}” ทุกตัวอักษร
					</span>
				</label>
			</div>
		{:else if starting || running}
			<div class="space-y-5 py-2" aria-live="polite">
				<div class="flex items-start gap-3 rounded-xl border bg-muted/30 p-4">
					<LoaderCircle class="mt-0.5 size-5 shrink-0 animate-spin text-destructive" />
					<div>
						<p class="font-medium">
							{viewPhase === 'finalizing'
								? 'กำลังลบข้อมูลในระบบ'
								: viewPhase === 'starting'
									? 'กำลังเริ่มลบกิจกรรม'
									: 'กำลังลบไฟล์และปิดการเข้าถึง'}
						</p>
						<p class="mt-1 text-sm text-muted-foreground">
							การลบทำงานต่อบนเซิร์ฟเวอร์ แม้ปิดหน้าต่างนี้
						</p>
					</div>
				</div>
				{#if status}
					<div class="space-y-2">
						<div class="flex justify-between text-sm">
							<span>ลบไฟล์แล้ว</span>
							<span class="tabular-nums">
								{formatCount(status.deletedFileCount)} / {formatCount(status.fileCount)}
							</span>
						</div>
						<Progress
							value={progressValue}
							max={progressMax}
							class="[&_[data-slot=progress-indicator]]:bg-destructive"
						/>
					</div>
				{/if}
			</div>
		{:else if viewPhase === 'failed'}
			<div class="space-y-4">
				<div class="flex gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 text-amber-950">
					<FileX2 class="mt-0.5 size-5 shrink-0" />
					<div>
						<p class="font-medium">การลบหยุดชั่วคราว</p>
						<p class="mt-1 text-sm text-amber-800">
							บางไฟล์ยังลบไม่สำเร็จ กิจกรรมยังถูกซ่อนและระบบจะไม่นำข้อมูลกลับมาเปิดใช้งาน
						</p>
					</div>
				</div>
				{#if status}
					<p class="text-sm text-muted-foreground">
						ลบไฟล์แล้ว {formatCount(status.deletedFileCount)} จาก {formatCount(status.fileCount)} ไฟล์
					</p>
				{/if}
				{#if errorMessage}
					<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
				{/if}
			</div>
		{:else if viewPhase === 'completed'}
			<div
				class="flex min-h-48 flex-col items-center justify-center gap-3 text-center"
				aria-live="polite"
			>
				<div
					class="flex size-12 items-center justify-center rounded-full bg-emerald-100 text-emerald-700"
				>
					<CheckCircle2 class="size-6" />
				</div>
				<div>
					<p class="font-medium">ลบกิจกรรมเรียบร้อยแล้ว</p>
					<p class="mt-1 text-sm text-muted-foreground">
						ข้อมูลและไฟล์ของกิจกรรมถูกนำออกจากระบบแล้ว
					</p>
				</div>
			</div>
		{:else}
			<div class="space-y-4">
				<div
					class="flex gap-3 rounded-xl border border-destructive/20 bg-destructive/5 p-4 text-destructive"
				>
					<AlertTriangle class="mt-0.5 size-5 shrink-0" />
					<div>
						<p class="font-medium">โหลดข้อมูลการลบไม่สำเร็จ</p>
						<p role="alert" class="mt-1 text-sm">{errorMessage}</p>
					</div>
				</div>
			</div>
		{/if}

		<Dialog.Footer>
			{#if viewPhase === 'confirm'}
				<Button variant="outline" onclick={() => changeOpen(false)}>ยกเลิก</Button>
				<LoadingButton
					variant="destructive"
					loading={starting}
					loadingLabel="กำลังเริ่มลบ..."
					disabled={!canConfirm}
					onclick={startPurge}
				>
					<Trash2 class="size-4" /> ลบกิจกรรมถาวร
				</LoadingButton>
			{:else if viewPhase === 'failed'}
				<Button variant="outline" onclick={() => changeOpen(false)}>ปิดหน้าต่าง</Button>
				<LoadingButton
					variant="destructive"
					loading={starting}
					loadingLabel="กำลังลองอีกครั้ง..."
					onclick={retryPurge}
				>
					<RefreshCw class="size-4" /> ลองลบต่อ
				</LoadingButton>
			{:else if viewPhase === 'load_error'}
				<Button variant="outline" onclick={() => changeOpen(false)}>ปิด</Button>
				<Button onclick={() => (observedPurge ? refreshStatus() : loadImpact())}>
					<RefreshCw class="size-4" /> ลองอีกครั้ง
				</Button>
			{:else if running}
				<Button variant="outline" onclick={() => changeOpen(false)}>ปิดหน้าต่าง</Button>
			{:else if viewPhase === 'completed'}
				<Button onclick={() => changeOpen(false)}>ปิด</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
