<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { ApiClientError } from '$lib/api/client';
	import {
		bulkUpdateCertificateCandidates,
		createAccountCertificateCandidate,
		createManualCertificateCandidate,
		deleteCertificateCandidate,
		getCertificateCampaign,
		importCertificateCandidates,
		listCertificateCandidates,
		listCertificateTemplates,
		submitCertificateIssueRequest,
		updateCertificateCandidate,
		type CertificateCampaignDetail,
		type CertificateCandidateBulkRequest,
		type CertificateCandidateDetail,
		type CertificateCandidateListResponse,
		type CertificateResourceLocked,
		type CertificateTemplateDetail,
		type CreateAccountCertificateCandidateRequest,
		type CreateManualExternalCandidateRequest,
		type UpdateCertificateCandidateRequest
	} from '$lib/api/certificates';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateAccountSearchDialog from './CertificateAccountSearchDialog.svelte';
	import CertificateCandidateEditDialog from './CertificateCandidateEditDialog.svelte';
	import CertificateCandidateTable from './CertificateCandidateTable.svelte';
	import CertificateImportDialog from './CertificateImportDialog.svelte';
	import CertificateManualExternalDialog from './CertificateManualExternalDialog.svelte';
	import CertificateSubmitRequestDialog from './CertificateSubmitRequestDialog.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import type { ParsedCertificateImport } from '$lib/certificates/importer';
	import {
		FileSpreadsheet,
		Filter,
		Search,
		Send,
		ShieldAlert,
		Sparkles,
		UserPlus,
		UsersRound
	} from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';

	let {
		campaignId,
		canReadCandidates
	}: {
		campaignId: string;
		canReadCandidates: boolean;
	} = $props();

	type ValidationStatus = CertificateCandidateDetail['validationStatus'];
	type CandidateSummary = CertificateCandidateListResponse['summary'];
	type StatusFilter = ValidationStatus | 'all';
	type ExternalConfirmationIssue = {
		candidateId: string;
		code: 'account_state_changed';
		message: string;
	};

	const emptySummary: CandidateSummary = {
		totalCount: 0,
		readyCount: 0,
		reviewCount: 0,
		invalidCount: 0
	};

	let campaign = $state.raw<CertificateCampaignDetail | null>(null);
	let templates = $state.raw<CertificateTemplateDetail[]>([]);
	let candidates = $state.raw<CertificateCandidateDetail[]>([]);
	let summary = $state.raw<CandidateSummary>({ ...emptySummary });
	let selectedIds = $state.raw<string[]>([]);
	let loading = $state(true);
	let tableLoading = $state(false);
	let error = $state('');
	let actionBusy = $state(false);
	let searchDraft = $state('');
	let searchQuery = $state('');
	let statusFilter = $state<StatusFilter>('all');
	let templateFilter = $state('');
	let bulkTemplateId = $state('');
	let importOpen = $state(false);
	let accountOpen = $state(false);
	let manualOpen = $state(false);
	let editTarget = $state.raw<CertificateCandidateDetail | null>(null);
	let deleteTarget = $state.raw<CertificateCandidateDetail | null>(null);
	let submitOpen = $state(false);
	let submitError = $state('');
	let lockedRequestId = $state<string | null>(null);
	let externalConfirmationIssues = $state.raw<ExternalConfirmationIssue[]>([]);
	let loadGeneration = 0;
	let candidateLoadGeneration = 0;
	let requestedCampaignId = '';

	const canPrepareCandidates = $derived(campaign?.capabilities.canPrepareCandidates === true);
	const canCreateCandidates = $derived(canPrepareCandidates);
	const canManageCandidates = $derived(canPrepareCandidates);
	const canDeleteCandidates = $derived(canManageCandidates);
	const canSubmitCandidates = $derived(campaign?.capabilities.canSubmit === true);
	const selectedCandidates = $derived(
		candidates.filter((candidate) => selectedIds.includes(candidate.id))
	);
	const canBulkUpdateSelection = $derived(
		selectedCandidates.length > 0 &&
			selectedCandidates.length === selectedIds.length &&
			selectedCandidates.every((candidate) => candidate.capabilities.canUpdate)
	);
	const canSubmitSelection = $derived(
		canSubmitCandidates &&
			selectedCandidates.length > 0 &&
			selectedCandidates.length === selectedIds.length &&
			selectedCandidates.every((candidate) => candidate.validationStatus === 'ready')
	);
	const canBulkConfirmExternal = $derived(
		selectedCandidates.length > 0 &&
			selectedCandidates.length === selectedIds.length &&
			selectedCandidates.every(
				(candidate) =>
					candidate.capabilities.canConfirmExternal &&
					candidate.matchStatus !== 'matched' &&
					candidate.matchStatus !== 'inactive' &&
					candidate.matchedUserId === null
			)
	);
	const canBulkConfirmDuplicate = $derived(
		selectedCandidates.length > 0 &&
			selectedCandidates.length === selectedIds.length &&
			selectedCandidates.every((candidate) => candidate.capabilities.canConfirmDuplicate)
	);
	const canBulkChooseName = $derived(
		selectedCandidates.length > 0 &&
			selectedCandidates.length === selectedIds.length &&
			selectedCandidates.every((candidate) => candidate.capabilities.canChooseName)
	);
	const bulkCompatibleTemplates = $derived(
		templates.filter(
			(template) =>
				template.isActive &&
				(selectedCandidates.length === 0 ||
					selectedCandidates.every((candidate) =>
						template.allowedRecipientTypes.includes(candidate.recipientType)
					))
		)
	);
	const bulkTemplateAllowed = $derived(
		canBulkUpdateSelection &&
			bulkTemplateId.length > 0 &&
			bulkCompatibleTemplates.some((template) => template.id === bulkTemplateId)
	);
	const recipientTypeLabels: Record<CertificateCandidateDetail['recipientType'], string> = {
		student: 'นักเรียน',
		staff: 'บุคลากร',
		external: 'บุคคลภายนอก'
	};

	function isCertificateResourceLocked(value: unknown): value is CertificateResourceLocked {
		if (value === null || typeof value !== 'object') return false;
		if (!('code' in value) || value.code !== 'resource_locked') return false;
		return (
			!('requestId' in value) || value.requestId === null || typeof value.requestId === 'string'
		);
	}

	function applyExternalConfirmationIssues(
		items: CertificateCandidateDetail[]
	): CertificateCandidateDetail[] {
		if (externalConfirmationIssues.length === 0) return items;
		const affectedIds = new Set(externalConfirmationIssues.map((issue) => issue.candidateId));
		return items.map((candidate) =>
			affectedIds.has(candidate.id)
				? {
						...candidate,
						validationStatus: 'needs_review',
						capabilities: {
							...candidate.capabilities,
							canConfirmExternal: false
						}
					}
				: candidate
		);
	}

	function clearExternalConfirmationIssues(candidateIds: string[]) {
		externalConfirmationIssues = externalConfirmationIssues.filter(
			(issue) => !candidateIds.includes(issue.candidateId)
		);
	}

	async function loadWorkspace(targetCampaignId: string) {
		const generation = ++loadGeneration;
		candidateLoadGeneration += 1;
		tableLoading = false;
		importOpen = false;
		accountOpen = false;
		manualOpen = false;
		editTarget = null;
		deleteTarget = null;
		submitOpen = false;
		submitError = '';
		lockedRequestId = null;
		externalConfirmationIssues = [];
		searchDraft = '';
		searchQuery = '';
		statusFilter = 'all';
		templateFilter = '';
		bulkTemplateId = '';
		if (!canReadCandidates) {
			if (generation === loadGeneration) loading = false;
			return;
		}
		loading = true;
		error = '';
		campaign = null;
		templates = [];
		candidates = [];
		summary = { ...emptySummary };
		selectedIds = [];
		try {
			const [loadedCampaign, loadedTemplates, loadedCandidates] = await Promise.all([
				getCertificateCampaign(targetCampaignId),
				listCertificateTemplates(targetCampaignId),
				listCertificateCandidates(targetCampaignId)
			]);
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			campaign = loadedCampaign;
			templates = loadedTemplates;
			candidates = applyExternalConfirmationIssues(loadedCandidates.items);
			summary = loadedCandidates.summary;
		} catch (loadError) {
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดรายชื่อผู้รับได้';
		} finally {
			if (generation === loadGeneration && targetCampaignId === campaignId) loading = false;
		}
	}

	async function loadCandidateList(targetCampaignId: string) {
		const generation = ++candidateLoadGeneration;
		tableLoading = true;
		try {
			const response = await listCertificateCandidates(targetCampaignId, {
				status: statusFilter === 'all' ? undefined : statusFilter,
				templateId: templateFilter || undefined,
				search: searchQuery || undefined
			});
			if (generation !== candidateLoadGeneration || targetCampaignId !== campaignId) return;
			candidates = applyExternalConfirmationIssues(response.items);
			summary = response.summary;
			selectedIds = selectedIds.filter((id) =>
				response.items.some((candidate) => candidate.id === id)
			);
		} catch (loadError) {
			if (generation !== candidateLoadGeneration || targetCampaignId !== campaignId) return;
			toast.error(loadError instanceof Error ? loadError.message : 'โหลดรายชื่อผู้รับไม่สำเร็จ');
		} finally {
			if (generation === candidateLoadGeneration && targetCampaignId === campaignId)
				tableLoading = false;
		}
	}

	function applySearch() {
		searchQuery = searchDraft.trim();
		void loadCandidateList(campaignId);
	}

	function setStatusFilter(status: StatusFilter) {
		if (statusFilter === status) return;
		statusFilter = status;
		selectedIds = [];
		void loadCandidateList(campaignId);
	}

	async function applyBulk(payload: CertificateCandidateBulkRequest, successMessage: string) {
		if (actionBusy || payload.candidateIds.length === 0) return;
		actionBusy = true;
		try {
			await bulkUpdateCertificateCandidates(campaignId, payload);
			clearExternalConfirmationIssues(payload.candidateIds);
			selectedIds = [];
			await loadCandidateList(campaignId);
			toast.success(successMessage);
		} catch (bulkError) {
			if (
				payload.operation === 'confirm_external' &&
				bulkError instanceof ApiClientError &&
				bulkError.status === 409
			) {
				const issues = payload.candidateIds.map((candidateId) => ({
					candidateId,
					code: 'account_state_changed' as const,
					message: bulkError.message
				}));
				const affectedIds = new Set(payload.candidateIds);
				externalConfirmationIssues = [
					...externalConfirmationIssues.filter((issue) => !affectedIds.has(issue.candidateId)),
					...issues
				];
				candidates = applyExternalConfirmationIssues(candidates);
				selectedIds = [];
				toast.error(bulkError.message);
				return;
			}
			await loadCandidateList(campaignId);
			toast.error(bulkError instanceof Error ? bulkError.message : 'ปรับปรุงรายชื่อไม่สำเร็จ');
		} finally {
			actionBusy = false;
		}
	}

	async function handleImport(parsed: ParsedCertificateImport) {
		if (!canCreateCandidates || actionBusy) return;
		actionBusy = true;
		try {
			const result = await importCertificateCandidates(campaignId, parsed);
			importOpen = false;
			await loadCandidateList(campaignId);
			toast.success(`นำเข้า ${result.batch.rowCount.toLocaleString('th-TH')} รายการแล้ว`);
		} catch (importError) {
			toast.error(importError instanceof Error ? importError.message : 'นำเข้ารายชื่อไม่สำเร็จ');
		} finally {
			actionBusy = false;
		}
	}

	async function handleAccountCreate(payload: CreateAccountCertificateCandidateRequest) {
		if (!canCreateCandidates || actionBusy) return;
		actionBusy = true;
		try {
			await createAccountCertificateCandidate(campaignId, payload);
			accountOpen = false;
			await loadCandidateList(campaignId);
			toast.success('เพิ่มผู้รับจากบัญชีแล้ว');
		} catch (createError) {
			toast.error(
				createError instanceof Error ? createError.message : 'เพิ่มผู้รับจากบัญชีไม่สำเร็จ'
			);
		} finally {
			actionBusy = false;
		}
	}

	async function handleManualCreate(payload: CreateManualExternalCandidateRequest) {
		if (!canCreateCandidates || actionBusy) return;
		actionBusy = true;
		try {
			await createManualCertificateCandidate(campaignId, payload);
			manualOpen = false;
			await loadCandidateList(campaignId);
			toast.success('เพิ่มบุคคลภายนอกแล้ว');
		} catch (createError) {
			toast.error(createError instanceof Error ? createError.message : 'เพิ่มบุคคลภายนอกไม่สำเร็จ');
		} finally {
			actionBusy = false;
		}
	}

	async function handleEdit(payload: UpdateCertificateCandidateRequest) {
		if (!editTarget || !canManageCandidates || actionBusy) return;
		actionBusy = true;
		try {
			const candidateId = editTarget.id;
			await updateCertificateCandidate(candidateId, payload);
			clearExternalConfirmationIssues([candidateId]);
			editTarget = null;
			await loadCandidateList(campaignId);
			toast.success('บันทึกรายชื่อแล้ว');
		} catch (updateError) {
			await loadCandidateList(campaignId);
			toast.error(updateError instanceof Error ? updateError.message : 'บันทึกรายชื่อไม่สำเร็จ');
		} finally {
			actionBusy = false;
		}
	}

	async function handleDelete() {
		if (!deleteTarget || !canDeleteCandidates || actionBusy) return;
		const target = deleteTarget;
		actionBusy = true;
		try {
			await deleteCertificateCandidate(target.id);
			clearExternalConfirmationIssues([target.id]);
			deleteTarget = null;
			selectedIds = selectedIds.filter((id) => id !== target.id);
			await loadCandidateList(campaignId);
			toast.success('ลบรายชื่อแล้ว');
		} catch (deleteError) {
			toast.error(deleteError instanceof Error ? deleteError.message : 'ลบรายชื่อไม่สำเร็จ');
		} finally {
			actionBusy = false;
		}
	}

	async function handleSubmitRequest(candidateIds: string[]) {
		if (!canSubmitSelection || actionBusy) return;
		actionBusy = true;
		submitError = '';
		lockedRequestId = null;
		try {
			await submitCertificateIssueRequest(campaignId, { candidateIds });
			selectedIds = [];
			submitOpen = false;
			await loadWorkspace(campaignId);
			toast.success('ส่งคำขอออกเกียรติบัตรแล้ว');
		} catch (submitFailure) {
			submitError =
				submitFailure instanceof Error ? submitFailure.message : 'ส่งคำขอออกเกียรติบัตรไม่สำเร็จ';
			if (
				submitFailure instanceof ApiClientError &&
				submitFailure.status === 409 &&
				isCertificateResourceLocked(submitFailure.data)
			) {
				lockedRequestId = submitFailure.data.requestId ?? null;
			}
			toast.error(submitError);
		} finally {
			actionBusy = false;
		}
	}

	function ensureWorkspaceLoaded() {
		if (!campaignId || requestedCampaignId === campaignId) return;
		requestedCampaignId = campaignId;
		void loadWorkspace(campaignId);
	}

	onMount(ensureWorkspaceLoaded);
	afterNavigate(ensureWorkspaceLoaded);
</script>

<PageShell
	title="ตรวจรายชื่อผู้รับ"
	description="นำเข้ารายชื่อ เชื่อมบัญชี แก้คำเตือน และกำหนดแบบให้พร้อมก่อนส่งคำขอออกเกียรติบัตร"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<UsersRound class="size-4" />
			{summary.totalCount.toLocaleString('th-TH')} รายการ{campaign ? ` · ${campaign.name}` : ''}
		</div>
	{/snippet}

	{#snippet actions()}
		{#if canCreateCandidates}
			<div class="flex flex-wrap gap-2">
				<Button variant="outline" onclick={() => (accountOpen = true)}>
					<UserPlus class="size-4" /> เพิ่มจากบัญชี
				</Button>
				<Button variant="outline" onclick={() => (manualOpen = true)}>
					<UserPlus class="size-4" /> เพิ่มบุคคลภายนอก
				</Button>
				<Button onclick={() => (importOpen = true)}>
					<FileSpreadsheet class="size-4" /> นำเข้า Excel/CSV
				</Button>
			</div>
		{/if}
	{/snippet}

	{#if !canReadCandidates}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรายชื่อผู้รับ"
			description="สิทธิ์การอ่านและขอบเขตหน่วยงานตรวจจาก backend"
		/>
	{:else if loading}
		<PageSkeleton variant="table" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดรายชื่อผู้รับไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(campaignId)}
		/>
	{:else}
		<section
			class="overflow-hidden rounded-xl border bg-card shadow-sm"
			aria-label="สรุปความพร้อมของรายชื่อ"
		>
			<div class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/25 px-5 py-3">
				<div>
					<p class="text-sm font-semibold">บัญชีตรวจสอบความพร้อม</p>
					<p class="text-xs text-muted-foreground">
						ต้องไม่มีรายการตรวจสอบหรือข้อมูลผิดก่อนส่งคำขอออก
					</p>
				</div>
				<span class="text-xs text-muted-foreground"
					>รวม {summary.totalCount.toLocaleString('th-TH')} รายการ</span
				>
			</div>
			<div class="grid sm:grid-cols-3">
				<button
					type="button"
					class={`group flex items-center justify-between gap-4 border-b px-5 py-4 text-left transition-colors hover:bg-emerald-50/70 sm:border-r sm:border-b-0 ${statusFilter === 'ready' ? 'bg-emerald-50' : ''}`}
					onclick={() => setStatusFilter(statusFilter === 'ready' ? 'all' : 'ready')}
				>
					<span>
						<span class="block text-sm font-semibold text-emerald-800">พร้อมออก</span>
						<span class="text-xs text-muted-foreground">ข้อมูลและแบบครบ</span>
					</span>
					<strong class="text-3xl tabular-nums text-emerald-700"
						>{summary.readyCount.toLocaleString('th-TH')}</strong
					>
				</button>
				<button
					type="button"
					class={`group flex items-center justify-between gap-4 border-b px-5 py-4 text-left transition-colors hover:bg-amber-50/70 sm:border-r sm:border-b-0 ${statusFilter === 'needs_review' ? 'bg-amber-50' : ''}`}
					onclick={() => setStatusFilter(statusFilter === 'needs_review' ? 'all' : 'needs_review')}
				>
					<span>
						<span class="block text-sm font-semibold text-amber-800">ต้องตรวจสอบ</span>
						<span class="text-xs text-muted-foreground">ต้องตัดสินใจหรือยืนยัน</span>
					</span>
					<strong class="text-3xl tabular-nums text-amber-700"
						>{summary.reviewCount.toLocaleString('th-TH')}</strong
					>
				</button>
				<button
					type="button"
					class={`group flex items-center justify-between gap-4 px-5 py-4 text-left transition-colors hover:bg-red-50/70 ${statusFilter === 'invalid' ? 'bg-red-50' : ''}`}
					onclick={() => setStatusFilter(statusFilter === 'invalid' ? 'all' : 'invalid')}
				>
					<span>
						<span class="block text-sm font-semibold text-red-800">ข้อมูลไม่ถูกต้อง</span>
						<span class="text-xs text-muted-foreground">ต้องแก้ข้อมูลต้นทาง</span>
					</span>
					<strong class="text-3xl tabular-nums text-red-700"
						>{summary.invalidCount.toLocaleString('th-TH')}</strong
					>
				</button>
			</div>
		</section>

		<section class="space-y-3 rounded-xl border bg-card p-3 sm:p-4">
			<div class="flex flex-wrap items-end gap-3">
				<label class="min-w-64 flex-1 space-y-1.5">
					<span class="text-xs font-medium text-muted-foreground"
						>ค้นหารายชื่อ รหัส หรือกิจกรรม</span
					>
					<div class="flex gap-2">
						<Input
							bind:value={searchDraft}
							placeholder="ค้นหาในรายชื่อผู้รับ"
							onkeydown={(event) => {
								if (event.key === 'Enter') applySearch();
							}}
						/>
						<Button size="icon" variant="outline" aria-label="ค้นหารายชื่อ" onclick={applySearch}>
							<Search class="size-4" />
						</Button>
					</div>
				</label>
				<label class="min-w-56 space-y-1.5">
					<span class="text-xs font-medium text-muted-foreground">กรองตามแบบ</span>
					<select
						bind:value={templateFilter}
						onchange={() => {
							selectedIds = [];
							void loadCandidateList(campaignId);
						}}
						class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
					>
						<option value="">ทุกแบบ</option>
						{#each templates as template (template.id)}
							<option value={template.id}>{template.name}</option>
						{/each}
					</select>
				</label>
				<div
					class="flex h-9 items-center gap-2 rounded-md border bg-muted/30 px-3 text-xs text-muted-foreground"
				>
					<Filter class="size-4" />
					{statusFilter === 'all' ? 'ทุกสถานะ' : 'กำลังกรองสถานะ'}
				</div>
			</div>

			{#if canManageCandidates || canSubmitCandidates}
				<div class="flex flex-wrap items-end gap-2 border-t pt-3">
					<div class="me-1 min-w-28 text-sm">
						<p class="font-semibold">เลือกแล้ว {selectedIds.length.toLocaleString('th-TH')}</p>
						<p class="text-xs text-muted-foreground">คำสั่งหลายรายการ</p>
					</div>
					{#if canManageCandidates}
						<label class="min-w-52 space-y-1">
							<span class="sr-only">แบบสำหรับรายการที่เลือก</span>
							<select
								bind:value={bulkTemplateId}
								class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
							>
								<option value="">เลือกแบบเกียรติบัตร</option>
								{#each bulkCompatibleTemplates as template (template.id)}
									<option value={template.id}>
										{template.name} · {template.allowedRecipientTypes
											.map((type) => recipientTypeLabels[type])
											.join('/')}
									</option>
								{/each}
							</select>
						</label>
						<LoadingButton
							variant="outline"
							loading={actionBusy}
							disabled={selectedIds.length === 0 || !bulkTemplateAllowed}
							onclick={() =>
								applyBulk(
									{
										candidateIds: selectedIds,
										operation: 'assign_template',
										templateId: bulkTemplateId
									},
									'กำหนดแบบให้รายการที่เลือกแล้ว'
								)}
						>
							กำหนดแบบให้รายการที่เลือก
						</LoadingButton>
						<Button
							variant="outline"
							disabled={!canBulkChooseName || actionBusy}
							onclick={() =>
								applyBulk(
									{ candidateIds: selectedIds, operation: 'choose_name', nameSource: 'account' },
									'ใช้ชื่อจากบัญชีแล้ว'
								)}
						>
							ใช้ชื่อจากบัญชี
						</Button>
						<Button
							variant="outline"
							disabled={!canBulkChooseName || actionBusy}
							onclick={() =>
								applyBulk(
									{ candidateIds: selectedIds, operation: 'choose_name', nameSource: 'file' },
									'ใช้ชื่อจากไฟล์แล้ว'
								)}
						>
							ใช้ชื่อจากไฟล์
						</Button>
						<Button
							variant="outline"
							disabled={!canBulkConfirmExternal || actionBusy}
							title={selectedIds.length > 0 && !canBulkConfirmExternal
								? 'เลือกเฉพาะรายการที่ไม่พบบัญชีและยืนยันได้ทั้งหมด'
								: undefined}
							onclick={() =>
								applyBulk(
									{ candidateIds: selectedIds, operation: 'confirm_external' },
									'ยืนยันเป็นบุคคลภายนอกแล้ว'
								)}
						>
							ยืนยันเป็นบุคคลภายนอก
						</Button>
						<Button
							variant="outline"
							disabled={!canBulkConfirmDuplicate || actionBusy}
							onclick={() =>
								applyBulk(
									{ candidateIds: selectedIds, operation: 'confirm_duplicate' },
									'ยืนยันรายชื่อซ้ำแล้ว'
								)}
						>
							ยืนยันรายชื่อซ้ำ
						</Button>
					{/if}
					{#if canSubmitCandidates}
						<Button
							disabled={!canSubmitSelection || actionBusy}
							title={selectedIds.length > 0 && !canSubmitSelection
								? 'เลือกเฉพาะรายการสถานะพร้อมออก'
								: undefined}
							onclick={() => {
								submitError = '';
								lockedRequestId = null;
								submitOpen = true;
							}}
						>
							<Send class="size-4" /> ส่งคำขอออกเกียรติบัตร
						</Button>
					{/if}
				</div>
			{/if}
		</section>

		{#if tableLoading}
			<div
				class="flex items-center justify-center gap-2 rounded-xl border py-10 text-sm text-muted-foreground"
			>
				<Sparkles class="size-4 animate-pulse" /> กำลังปรับรายการตามตัวกรอง...
			</div>
		{:else}
			<CertificateCandidateTable
				{candidates}
				{selectedIds}
				{externalConfirmationIssues}
				canManage={canManageCandidates}
				canSubmit={canSubmitCandidates}
				canDelete={canDeleteCandidates}
				onselectionchange={(ids) => (selectedIds = ids)}
				onedit={(candidate) => (editTarget = candidate)}
				onchoosename={(candidate, source) =>
					applyBulk(
						{ candidateIds: [candidate.id], operation: 'choose_name', nameSource: source },
						'เลือกชื่อที่จะใช้แล้ว'
					)}
				onconfirmexternal={(candidate) =>
					applyBulk(
						{ candidateIds: [candidate.id], operation: 'confirm_external' },
						'ยืนยันเป็นบุคคลภายนอกแล้ว'
					)}
				onconfirmduplicate={(candidate) =>
					applyBulk(
						{ candidateIds: [candidate.id], operation: 'confirm_duplicate' },
						'ยืนยันรายชื่อซ้ำแล้ว'
					)}
				ondelete={(candidate) => (deleteTarget = candidate)}
			/>
		{/if}
	{/if}
</PageShell>

{#if importOpen}
	<CertificateImportDialog
		open={importOpen}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (importOpen = open)}
		onimport={handleImport}
	/>
{/if}

{#if accountOpen}
	<CertificateAccountSearchDialog
		open={accountOpen}
		{campaignId}
		{templates}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (accountOpen = open)}
		oncreate={handleAccountCreate}
	/>
{/if}

{#if manualOpen}
	<CertificateManualExternalDialog
		open={manualOpen}
		{templates}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (manualOpen = open)}
		oncreate={handleManualCreate}
	/>
{/if}

{#if editTarget}
	{#key editTarget.id}
		<CertificateCandidateEditDialog
			open={true}
			candidate={editTarget}
			{templates}
			busy={actionBusy}
			onopenchange={(open) => !open && !actionBusy && (editTarget = null)}
			onsave={handleEdit}
		/>
	{/key}
{/if}

{#if submitOpen && campaign}
	<CertificateSubmitRequestDialog
		open={submitOpen}
		{campaignId}
		campaignName={campaign.name}
		candidates={selectedCandidates}
		busy={actionBusy}
		error={submitError}
		{lockedRequestId}
		onopenchange={(open) => {
			if (actionBusy) return;
			submitOpen = open;
			if (!open) {
				submitError = '';
				lockedRequestId = null;
			}
		}}
		onsubmit={handleSubmitRequest}
	/>
{/if}

<AlertDialog.Root
	open={deleteTarget !== null}
	onOpenChange={(open) => !open && !actionBusy && (deleteTarget = null)}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ลบรายชื่อผู้รับนี้?</AlertDialog.Title>
			<AlertDialog.Description>
				รายการของ {deleteTarget?.importedFirstName ?? ''}
				{deleteTarget?.importedLastName ?? ''}
				จะถูกนำออกจากชุดเตรียมออกเกียรติบัตร และสามารถนำเข้าใหม่ภายหลังได้
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={actionBusy}>ยกเลิก</AlertDialog.Cancel>
			<LoadingButton variant="destructive" loading={actionBusy} onclick={handleDelete}>
				<ShieldAlert class="size-4" /> ลบรายชื่อ
			</LoadingButton>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
