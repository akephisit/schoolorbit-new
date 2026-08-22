<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		createCertificateTemplatePreviewManifest,
		getCertificateIssueRequest,
		issueCertificates,
		returnCertificateIssueRequest,
		startCertificateIssueRequestReview,
		type CertificateIssueCode,
		type CertificateRenderManifest,
		type CertificateIssueRequestDetail,
		type CertificateIssueRequestItem,
		type CertificateIssueRequestStatus,
		type IssueCertificateOutcome
	} from '$lib/api/certificates';
	import type { CertificatePreviewState } from '$lib/certificates/preview-fit';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateIssueConfirmationDialog from '$lib/components/certificates/CertificateIssueConfirmationDialog.svelte';
	import CertificatePreviewDialog from '$lib/components/certificates/CertificatePreviewDialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Table from '$lib/components/ui/table';
	import { Textarea } from '$lib/components/ui/textarea';
	import {
		AlertTriangle,
		ArrowLeft,
		Award,
		CircleCheckBig,
		Eye,
		FileCheck2,
		RotateCcw,
		ShieldCheck,
		UsersRound
	} from 'lucide-svelte';
	import { onDestroy, onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	let {
		requestId,
		canIssue
	}: {
		requestId: string;
		canIssue: boolean;
	} = $props();

	const statusMeta: Record<CertificateIssueRequestStatus, { label: string; className: string }> = {
		pending: { label: 'รอตรวจ', className: 'border-blue-200 bg-blue-50 text-blue-800' },
		reviewing: { label: 'กำลังตรวจ', className: 'border-amber-200 bg-amber-50 text-amber-800' },
		returned: { label: 'ส่งกลับแล้ว', className: 'border-orange-200 bg-orange-50 text-orange-800' },
		withdrawn: { label: 'ถอนแล้ว', className: 'border-slate-200 bg-slate-50 text-slate-700' },
		issued: { label: 'ออกแล้ว', className: 'border-emerald-200 bg-emerald-50 text-emerald-800' }
	};

	const issueOptions: Array<{ code: CertificateIssueCode; label: string }> = [
		{ code: 'candidate_not_ready', label: 'ข้อมูลผู้รับไม่พร้อม' },
		{ code: 'account_state_changed', label: 'สถานะบัญชีเปลี่ยนจากตอนเตรียม' },
		{ code: 'template_not_ready', label: 'แบบเกียรติบัตรยังไม่พร้อม' },
		{ code: 'template_incompatible', label: 'แบบไม่รองรับประเภทผู้รับ' },
		{ code: 'asset_unavailable', label: 'รูปภาพหรือฟอนต์ในแบบไม่พร้อม' },
		{ code: 'campaign_unavailable', label: 'ข้อมูลกิจกรรมไม่พร้อม' },
		{ code: 'reviewer_requested_changes', label: 'ผู้ตรวจขอให้แก้ไข' }
	];

	const recipientLabels: Record<CertificateIssueRequestItem['recipientType'], string> = {
		student: 'นักเรียน',
		staff: 'บุคลากร',
		external: 'บุคคลภายนอก'
	};

	let request = $state.raw<CertificateIssueRequestDetail | null>(null);
	let loading = $state(true);
	let error = $state('');
	let actionBusy = $state(false);
	let returnCodes = $state.raw<CertificateIssueCode[]>([]);
	let returnNote = $state('');
	let requestedRequestId = '';
	let loadGeneration = 0;
	let previewOpen = $state(false);
	let previewState = $state<CertificatePreviewState>('idle');
	let previewItem = $state.raw<CertificateIssueRequestItem | null>(null);
	let previewManifest = $state.raw<CertificateRenderManifest | null>(null);
	let previewManifestLoading = $state(false);
	let previewManifestError = $state('');
	let previewController: AbortController | null = null;
	let issueDialogOpen = $state(false);
	let issueError = $state('');
	let issueAttemptKey: string | null = null;
	type IssuedOutcome = Extract<IssueCertificateOutcome, { outcome: 'issued' }>;
	type ReturnedOutcome = Extract<IssueCertificateOutcome, { outcome: 'returned' }>;
	let issuedOutcome = $state.raw<IssuedOutcome | null>(null);
	let issueReturnedOutcome = $state.raw<ReturnedOutcome | null>(null);
	const previewing = $derived(previewState === 'loading' || previewManifestLoading);

	const canReturn = $derived(
		request?.capabilities.canReturn === true &&
			returnCodes.length > 0 &&
			returnNote.trim().length > 0
	);

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short',
			timeZone: 'Asia/Bangkok'
		}).format(new Date(value));
	}

	function displayName(item: CertificateIssueRequestItem): string {
		return `${item.title ?? ''}${item.firstName} ${item.lastName}`.trim();
	}

	function isCurrentRequest(targetRequestId: string, targetGeneration: number): boolean {
		return (
			targetGeneration === loadGeneration &&
			targetRequestId === requestId &&
			request?.id === targetRequestId
		);
	}

	function toggleIssueCode(code: CertificateIssueCode, checked: boolean) {
		returnCodes = checked
			? Array.from(new Set([...returnCodes, code]))
			: returnCodes.filter((current) => current !== code);
	}

	async function loadRequest(targetRequestId: string) {
		const generation = ++loadGeneration;
		actionBusy = false;
		if (!canIssue) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		request = null;
		returnCodes = [];
		returnNote = '';
		issueDialogOpen = false;
		issueError = '';
		issueAttemptKey = null;
		issuedOutcome = null;
		issueReturnedOutcome = null;
		closePreview();
		try {
			const loaded = await getCertificateIssueRequest(targetRequestId);
			if (generation !== loadGeneration || targetRequestId !== requestId) return;
			request = loaded;
		} catch (loadError) {
			if (generation !== loadGeneration || targetRequestId !== requestId) return;
			error = loadError instanceof Error ? loadError.message : 'โหลดคำขอไม่สำเร็จ';
		} finally {
			if (generation === loadGeneration && targetRequestId === requestId) loading = false;
		}
	}

	async function startReview() {
		if (!request?.capabilities.canStartReview || actionBusy) return;
		const targetRequestId = request.id;
		const targetGeneration = loadGeneration;
		actionBusy = true;
		try {
			const updated = await startCertificateIssueRequestReview(targetRequestId);
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			request = updated;
			toast.success('เริ่มตรวจคำขอแล้ว');
		} catch (reviewError) {
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			toast.error(reviewError instanceof Error ? reviewError.message : 'เริ่มตรวจคำขอไม่สำเร็จ');
		} finally {
			if (isCurrentRequest(targetRequestId, targetGeneration)) actionBusy = false;
		}
	}

	async function returnRequest() {
		if (!request || !canReturn || actionBusy) return;
		const targetRequestId = request.id;
		const targetGeneration = loadGeneration;
		actionBusy = true;
		try {
			const updated = await returnCertificateIssueRequest(targetRequestId, {
				issueCodes: returnCodes,
				returnNote: returnNote.trim()
			});
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			request = updated;
			returnCodes = [];
			returnNote = '';
			toast.success('ส่งกลับให้หน่วยงานแก้ไขแล้ว');
		} catch (returnError) {
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			toast.error(returnError instanceof Error ? returnError.message : 'ส่งกลับคำขอไม่สำเร็จ');
		} finally {
			if (isCurrentRequest(targetRequestId, targetGeneration)) actionBusy = false;
		}
	}

	function openIssueConfirmation() {
		if (!request?.capabilities.canIssue || actionBusy) return;
		issueError = '';
		issueDialogOpen = true;
	}

	async function confirmIssue() {
		if (!request?.capabilities.canIssue || actionBusy) return;
		const targetRequestId = request.id;
		const targetGeneration = loadGeneration;
		issueAttemptKey ??= crypto.randomUUID();
		actionBusy = true;
		issueError = '';
		try {
			const outcome = await issueCertificates(targetRequestId, {
				idempotencyKey: issueAttemptKey
			});
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			if (outcome.outcome === 'issued') {
				issueAttemptKey = null;
				issueDialogOpen = false;
				issuedOutcome = outcome;
				issueReturnedOutcome = null;
				request = {
					...request,
					status: 'issued',
					issuedAt: outcome.certificates[0]?.createdAt ?? new Date().toISOString(),
					capabilities: {
						canWithdraw: false,
						canStartReview: false,
						canReturn: false,
						canIssue: false
					}
				};
				toast.success(
					`ออกเลขเกียรติบัตรแล้ว ${outcome.certificates.length.toLocaleString('th-TH')} ใบ`
				);
			} else if (outcome.outcome === 'returned') {
				issueAttemptKey = null;
				issueDialogOpen = false;
				issuedOutcome = null;
				issueReturnedOutcome = outcome;
				request = {
					...request,
					status: 'returned',
					returnedAt: new Date().toISOString(),
					issueCodes: outcome.issueCodes,
					returnNote: null,
					capabilities: {
						canWithdraw: false,
						canStartReview: false,
						canReturn: false,
						canIssue: false
					}
				};
				toast.warning('ข้อมูลเปลี่ยนระหว่างตรวจ ระบบส่งคำขอกลับโดยยังไม่ออกเลข');
			} else {
				throw new Error('ระบบตอบกลับผลการออกเลขในรูปแบบที่ไม่รองรับ กรุณาลองอีกครั้ง');
			}
		} catch (issueFailure) {
			if (!isCurrentRequest(targetRequestId, targetGeneration)) return;
			issueError =
				issueFailure instanceof Error ? issueFailure.message : 'ยืนยันการออกเลขไม่สำเร็จ';
		} finally {
			if (isCurrentRequest(targetRequestId, targetGeneration)) actionBusy = false;
		}
	}

	async function preview(item: CertificateIssueRequestItem) {
		if (!canIssue || previewing || !item.templateId) return;
		previewController?.abort();
		const controller = new AbortController();
		previewController = controller;
		previewItem = item;
		previewOpen = true;
		previewState = 'loading';
		previewManifest = null;
		previewManifestLoading = true;
		previewManifestError = '';
		try {
			const manifest = await createCertificateTemplatePreviewManifest(
				item.templateId,
				{
					previewKind: 'candidate',
					candidateId: item.candidateId
				},
				{ signal: controller.signal }
			);
			controller.signal.throwIfAborted();
			previewManifest = manifest;
		} catch (previewFailure) {
			if (controller.signal.aborted || previewController !== controller) return;
			previewManifestError =
				previewFailure instanceof Error ? previewFailure.message : 'สร้างตัวอย่างไม่สำเร็จ';
			previewState = 'error';
		} finally {
			if (previewController === controller) {
				previewController = null;
				previewManifestLoading = false;
			}
		}
	}

	function retryPreview() {
		if (previewState !== 'error' || !previewItem) return;
		previewState = 'idle';
		void preview(previewItem);
	}

	function closePreview() {
		previewController?.abort();
		previewController = null;
		previewOpen = false;
		previewState = 'idle';
		previewItem = null;
		previewManifest = null;
		previewManifestLoading = false;
		previewManifestError = '';
	}

	function ensureLoaded() {
		if (!requestId || requestedRequestId === requestId) return;
		requestedRequestId = requestId;
		void loadRequest(requestId);
	}

	onMount(ensureLoaded);
	afterNavigate(ensureLoaded);
	onDestroy(() => previewController?.abort());
</script>

<PageShell
	title="ตรวจคำขอออกเกียรติบัตร"
	description="ตรวจข้อมูลผู้รับและตัวอย่างจากข้อมูลจริง หน้านี้ไม่แก้รายชื่อ กิจกรรม หรือแบบเกียรติบัตร"
>
	{#snippet meta()}
		{#if request}
			<Badge variant="outline" class={statusMeta[request.status].className}>
				{statusMeta[request.status].label}
			</Badge>
		{/if}
	{/snippet}

	{#snippet actions()}
		<div class="flex flex-wrap gap-2">
			<Button variant="outline" href={resolve('/staff/certificate-requests')}>
				<ArrowLeft class="size-4" /> กลับคิวคำขอ
			</Button>
			{#if request?.capabilities.canStartReview}
				<LoadingButton loading={actionBusy} onclick={startReview}>
					<ShieldCheck class="size-4" /> เริ่มตรวจคำขอ
				</LoadingButton>
			{/if}
			{#if request?.capabilities.canIssue}
				<Button disabled={actionBusy} onclick={openIssueConfirmation}>
					<Award class="size-4" /> ออกเกียรติบัตร {request.itemCount.toLocaleString('th-TH')} ใบ
				</Button>
			{/if}
		</div>
	{/snippet}

	{#if !canIssue}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์เปิดรายละเอียดคำขอ"
			description="รายชื่อผู้รับโหลดได้หลังผ่านสิทธิ์ออกเกียรติบัตรระดับโรงเรียนเท่านั้น"
		/>
	{:else if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดคำขอไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadRequest(requestId)}
		/>
	{:else if request}
		<div class="space-y-5">
			{#if issuedOutcome}
				<section class="overflow-hidden rounded-xl border border-emerald-200 bg-emerald-50">
					<div class="flex items-start gap-3 p-5 text-emerald-950">
						<CircleCheckBig class="mt-0.5 size-6 shrink-0 text-emerald-700" />
						<div class="min-w-0 flex-1">
							<h2 class="text-lg font-semibold">ออกเลขแล้ว</h2>
							<p class="mt-1 text-sm text-emerald-800">
								ออกสำเร็จ {issuedOutcome.certificates.length.toLocaleString('th-TH')} ใบ เลขทุกใบถูกบันทึกและจะไม่ถูกนำกลับมาใช้
							</p>
							<div
								class="mt-4 grid gap-px overflow-hidden rounded-lg border border-emerald-200 bg-emerald-200 sm:grid-cols-2"
							>
								<div class="bg-white p-3">
									<p class="text-xs text-emerald-700">เลขใบแรก</p>
									<p class="mt-1 font-mono font-semibold tabular-nums">
										{issuedOutcome.certificates[0]?.certificateNumber ?? '-'}
									</p>
								</div>
								<div class="bg-white p-3">
									<p class="text-xs text-emerald-700">เลขใบสุดท้าย</p>
									<p class="mt-1 font-mono font-semibold tabular-nums">
										{issuedOutcome.certificates.at(-1)?.certificateNumber ?? '-'}
									</p>
								</div>
							</div>
							<a
								href={resolve(
									`/staff/certificates/${request.campaignId}/issued` as '/staff/certificates/[campaignId]/issued'
								)}
								class="mt-4 inline-flex items-center font-medium text-emerald-900 underline underline-offset-4"
							>
								เปิดทะเบียนใบที่ออกแล้ว
							</a>
						</div>
					</div>
				</section>
			{:else if issueReturnedOutcome}
				<section class="rounded-xl border border-amber-200 bg-amber-50 p-5 text-amber-950">
					<div class="flex items-start gap-3">
						<AlertTriangle class="mt-0.5 size-5 shrink-0" />
						<div>
							<h2 class="font-semibold">ระบบส่งคำขอกลับโดยยังไม่ออกเลข</h2>
							<p class="mt-1 text-sm text-amber-800">
								ผลตรวจล่าสุดพบข้อมูลเปลี่ยนแปลง ยังไม่มีเลขเกียรติบัตรถูกจอง
								ให้หน่วยงานแก้รายการด้านล่างแล้วส่งคำขอใหม่
							</p>
						</div>
					</div>
					<div class="mt-4 space-y-2">
						{#each issueReturnedOutcome.candidateProblems as problem (problem.candidateId)}
							{@const problemItem = request.items.find(
								(item) => item.candidateId === problem.candidateId
							)}
							<div class="rounded-lg border border-amber-200 bg-white p-3 text-sm">
								<p class="font-medium">{problemItem ? displayName(problemItem) : 'รายการผู้รับ'}</p>
								<div class="mt-2 flex flex-wrap gap-1.5">
									{#each problem.issueCodes as code (code)}
										<Badge variant="outline" class="border-amber-300 bg-amber-50 text-amber-900">
											{issueOptions.find((option) => option.code === code)?.label ?? code}
										</Badge>
									{/each}
								</div>
							</div>
						{/each}
					</div>
				</section>
			{/if}

			<section class="overflow-hidden rounded-xl border bg-card shadow-sm">
				<div class="grid gap-px bg-border sm:grid-cols-[1.4fr_1fr_1fr]">
					<div class="bg-card p-5">
						<p class="text-xs font-medium text-muted-foreground">กิจกรรม</p>
						<h2 class="mt-1 text-lg font-semibold">{request.campaignName}</h2>
						<p class="mt-1 text-sm text-muted-foreground">
							{request.ownerOrganizationUnitName ?? 'ระดับโรงเรียน'}
						</p>
					</div>
					<div class="bg-card p-5">
						<p class="text-xs font-medium text-muted-foreground">ผู้ส่งคำขอ</p>
						<p class="mt-1 font-medium">{request.submittedByName}</p>
						<p class="mt-1 text-xs text-muted-foreground">{formatDate(request.submittedAt)}</p>
					</div>
					<div class="bg-card p-5">
						<p class="text-xs font-medium text-muted-foreground">ขอบเขตคำขอ</p>
						<p class="mt-1 font-medium">
							{request.itemCount.toLocaleString('th-TH')} รายชื่อ · {request.templateCount.toLocaleString(
								'th-TH'
							)} แบบ
						</p>
						<p class="mt-1 text-xs text-muted-foreground">
							ประเมินความพร้อมอีกครั้งจากข้อมูลปัจจุบัน
						</p>
					</div>
				</div>
				<div class="grid grid-cols-3 border-t text-center">
					<div class="p-3 text-emerald-800">
						<strong class="block text-xl tabular-nums"
							>{request.readyCount.toLocaleString('th-TH')}</strong
						>
						<span class="text-xs">พร้อมออก</span>
					</div>
					<div class="border-x p-3 text-amber-800">
						<strong class="block text-xl tabular-nums"
							>{request.reviewCount.toLocaleString('th-TH')}</strong
						>
						<span class="text-xs">ต้องตรวจสอบ</span>
					</div>
					<div class="p-3 text-red-800">
						<strong class="block text-xl tabular-nums"
							>{request.invalidCount.toLocaleString('th-TH')}</strong
						>
						<span class="text-xs">ข้อมูลไม่ถูกต้อง</span>
					</div>
				</div>
			</section>

			<div
				class="flex items-start gap-3 rounded-xl border border-blue-200 bg-blue-50 p-4 text-sm text-blue-950"
			>
				<FileCheck2 class="mt-0.5 size-4 shrink-0" />
				<div>
					<p class="font-medium">หน้าตรวจเป็นแบบอ่านอย่างเดียว</p>
					<p class="mt-1 text-blue-800">
						หากพบข้อมูลผิด ให้เลือกรหัสเหตุผลและส่งกลับ
						หน่วยงานจะแก้ในพื้นที่เตรียมแล้วสร้างคำขอใหม่
					</p>
				</div>
			</div>

			<section class="space-y-3">
				<div class="flex items-center justify-between gap-3">
					<div>
						<h2 class="font-semibold">รายชื่อในคำขอ</h2>
						<p class="text-xs text-muted-foreground">เปิดดูตัวอย่างได้ทีละรายการ</p>
					</div>
					<div class="flex items-center gap-2 text-xs text-muted-foreground">
						<UsersRound class="size-4" />
						{request.items.length.toLocaleString('th-TH')} รายการ
					</div>
				</div>
				<div class="overflow-x-auto rounded-xl border bg-card shadow-sm">
					<Table.Root class="min-w-[980px]">
						<Table.Header>
							<Table.Row class="bg-muted/40 hover:bg-muted/40">
								<Table.Head class="w-56">ผู้รับ</Table.Head>
								<Table.Head class="w-32">ประเภท</Table.Head>
								<Table.Head class="w-52">กิจกรรม / รางวัล</Table.Head>
								<Table.Head class="w-52">แบบ</Table.Head>
								<Table.Head class="w-36">สถานะปัจจุบัน</Table.Head>
								<Table.Head class="w-28 text-right">ตัวอย่าง</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each request.items as item (item.candidateId)}
								<Table.Row>
									<Table.Cell class="font-medium">{displayName(item)}</Table.Cell>
									<Table.Cell>{recipientLabels[item.recipientType]}</Table.Cell>
									<Table.Cell>
										<p>{item.activityItem ?? '-'}</p>
										{#if item.awardOrRole}
											<p class="mt-1 text-xs text-muted-foreground">{item.awardOrRole}</p>
										{/if}
									</Table.Cell>
									<Table.Cell>{item.templateName ?? 'ไม่พบแบบ'}</Table.Cell>
									<Table.Cell>
										{#if item.validationStatus === 'ready'}
											<Badge
												variant="outline"
												class="border-emerald-200 bg-emerald-50 text-emerald-800">พร้อมออก</Badge
											>
										{:else if item.validationStatus === 'needs_review'}
											<Badge variant="outline" class="border-amber-200 bg-amber-50 text-amber-800"
												>ต้องตรวจสอบ</Badge
											>
										{:else}
											<Badge variant="outline" class="border-red-200 bg-red-50 text-red-800"
												>ข้อมูลไม่ถูกต้อง</Badge
											>
										{/if}
									</Table.Cell>
									<Table.Cell class="text-right">
										<Button
											size="sm"
											variant="outline"
											disabled={!item.templateId || previewing}
											onclick={() => preview(item)}
										>
											<Eye class="size-4" /> ดูตัวอย่าง
										</Button>
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			</section>

			{#if request.capabilities.canReturn}
				<section class="rounded-xl border border-orange-200 bg-orange-50/60 p-5">
					<div class="flex items-start gap-3">
						<RotateCcw class="mt-0.5 size-5 text-orange-700" />
						<div>
							<h2 class="font-semibold text-orange-950">ส่งกลับให้แก้ไข</h2>
							<p class="mt-1 text-sm text-orange-800">
								เลือกรหัสเหตุผลอย่างน้อยหนึ่งข้อ และเขียนเฉพาะคำแนะนำที่จำเป็น
								ห้ามใส่เลขประจำตัวประชาชน
							</p>
						</div>
					</div>
					<div class="mt-4 grid gap-2 sm:grid-cols-2">
						{#each issueOptions as option (option.code)}
							<label
								class="flex items-start gap-2 rounded-lg border border-orange-200 bg-white p-3 text-sm"
							>
								<input
									type="checkbox"
									class="mt-0.5 size-4 rounded border-input accent-primary"
									checked={returnCodes.includes(option.code)}
									onchange={(event) => toggleIssueCode(option.code, event.currentTarget.checked)}
								/>
								<span>{option.label}</span>
							</label>
						{/each}
					</div>
					<label class="mt-4 block space-y-1.5">
						<span class="text-sm font-medium text-orange-950">หมายเหตุส่งกลับ</span>
						<Textarea
							bind:value={returnNote}
							maxlength={500}
							rows={4}
							placeholder="ระบุสิ่งที่ต้องแก้ให้ชัดเจน โดยไม่ใส่ข้อมูลอ่อนไหว"
						/>
						<span class="block text-right text-xs text-orange-800">
							{returnNote.length.toLocaleString('th-TH')}/500
						</span>
					</label>
					<div class="mt-4 flex justify-end">
						<LoadingButton loading={actionBusy} disabled={!canReturn} onclick={returnRequest}>
							<RotateCcw class="size-4" /> ส่งกลับให้แก้ไข
						</LoadingButton>
					</div>
				</section>
			{:else if request.status === 'returned' && !issueReturnedOutcome}
				<section class="rounded-xl border border-orange-200 bg-orange-50 p-5 text-orange-950">
					<div class="flex items-center gap-2">
						<RotateCcw class="size-5" />
						<h2 class="font-semibold">ส่งกลับแล้ว</h2>
					</div>
					<div class="mt-3 flex flex-wrap gap-1.5">
						{#each request.issueCodes as code (code)}
							<Badge variant="outline" class="border-orange-300 bg-white text-orange-900">
								{issueOptions.find((option) => option.code === code)?.label ?? code}
							</Badge>
						{/each}
					</div>
					{#if request.returnNote}<p class="mt-3 text-sm">{request.returnNote}</p>{/if}
				</section>
			{/if}
		</div>
	{/if}
</PageShell>

{#if request}
	<CertificateIssueConfirmationDialog
		open={issueDialogOpen}
		{request}
		busy={actionBusy}
		error={issueError}
		onopenchange={(open) => (issueDialogOpen = open)}
		onconfirm={confirmIssue}
		onpreview={preview}
	/>
{/if}

<CertificatePreviewDialog
	open={previewOpen}
	title="ตัวอย่างเกียรติบัตร"
	description={`${previewItem ? displayName(previewItem) : ''} · ข้อมูลพรีวิว ไม่ใช่เกียรติบัตรที่ออกเลขแล้ว`}
	manifest={previewManifest}
	manifestLoading={previewManifestLoading}
	manifestError={previewManifestError}
	ariaLabel="ตัวอย่างเกียรติบัตรสำหรับตรวจคำขอ"
	loadingLabel="กำลังโหลดฟอนต์และสร้างตัวอย่าง…"
	renderFailureMessage="สร้างตัวอย่างเกียรติบัตรไม่สำเร็จ"
	onretry={retryPreview}
	onstatechange={(state) => (previewState = state)}
	onopenchange={(open) => !open && closePreview()}
/>
