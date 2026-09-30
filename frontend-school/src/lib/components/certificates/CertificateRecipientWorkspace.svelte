<script lang="ts">
	import { pushState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
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
	import * as Select from '$lib/components/ui/select';
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
	} from '@lucide/svelte';
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';

	const ALL_TEMPLATES_VALUE = '__all_templates__';
	const NO_BULK_TEMPLATE_VALUE = '__no_bulk_template__';

	let {
		campaignId,
		canReadCandidates,
		canUpdate,
		canSubmit,
		identityKey,
		initialCampaign,
		initialTemplates,
		initialCandidates
	}: {
		campaignId: string;
		canReadCandidates: boolean;
		canUpdate: boolean;
		canSubmit: boolean;
		identityKey: string;
		initialCampaign: Promise<
			RouteLoadResult<{ ownerKey: string; record: CertificateCampaignDetail | null }>
		>;
		initialTemplates: Promise<
			RouteLoadResult<{ ownerKey: string; record: CertificateTemplateDetail[] | null }>
		>;
		initialCandidates: Promise<
			RouteLoadResult<{
				ownerKey: string;
				filterKey: string;
				record: CertificateCandidateListResponse | null;
			}>
		>;
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
	const context = $derived(`${identityKey}|${campaignId}`);
	const currentUrl = $derived(
		new URL(page.state.certificateCandidateUrl ?? page.url.href, page.url)
	);
	const candidateFilterKey = $derived(
		currentUrl.searchParams.get('status') +
			'|' +
			currentUrl.searchParams.get('templateId') +
			'|' +
			currentUrl.searchParams.get('search')
	);
	let owner = '',
		ownerEpoch = 0,
		disposed = false;
	let campaignLoading = $state(true),
		campaignError = $state(''),
		templatesLoading = $state(true),
		templatesError = $state(''),
		loaded = $state(false);
	const campaignRequest = new LatestRequest(),
		templateRequest = new LatestRequest(),
		candidateRequest = new LatestRequest();
	let consumedCampaign: typeof initialCampaign | null = null,
		consumedTemplates: typeof initialTemplates | null = null,
		consumedCandidates: typeof initialCandidates | null = null;
	let appliedFilter = '';
	$effect.pre(() => {
		const key = context,
			a = initialCampaign,
			b = initialTemplates,
			c = initialCandidates,
			allowed = canReadCandidates,
			filter = candidateFilterKey;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				campaignRequest.abort();
				templateRequest.abort();
				candidateRequest.abort();
				campaign = null;
				templates = [];
				candidates = [];
				summary = { ...emptySummary };
				selectedIds = [];
				loaded = false;
				loading = allowed;
				tableLoading = false;
				campaignLoading = allowed;
				templatesLoading = allowed;
				campaignError = '';
				templatesError = '';
				error = '';
				actionBusy = false;
				importOpen = false;
				accountOpen = false;
				manualOpen = false;
				editTarget = null;
				deleteTarget = null;
				submitOpen = false;
				submitError = '';
				lockedRequestId = null;
				externalConfirmationIssues = [];
			}
			statusFilter =
				currentUrl.searchParams.get('status') === 'ready'
					? 'ready'
					: currentUrl.searchParams.get('status') === 'needs_review'
						? 'needs_review'
						: currentUrl.searchParams.get('status') === 'invalid'
							? 'invalid'
							: 'all';
			templateFilter = currentUrl.searchParams.get('templateId') ?? '';
			searchQuery = currentUrl.searchParams.get('search') ?? '';
			searchDraft = searchQuery;
			if (!allowed) return;
			if (a !== consumedCampaign) {
				consumedCampaign = a;
				const t = campaignRequest.begin();
				campaignLoading = true;
				void a.then((v) => applyCampaign(v, t.revision));
			}
			if (b !== consumedTemplates) {
				consumedTemplates = b;
				const t = templateRequest.begin();
				templatesLoading = true;
				void b.then((v) => applyTemplates(v, t.revision));
			}
			if (c !== consumedCandidates) {
				consumedCandidates = c;
				appliedFilter = filter;
				const t = candidateRequest.begin();
				loading = true;
				void c.then((v) => applyCandidates(v, t.revision));
			} else if (appliedFilter !== filter) {
				appliedFilter = filter;
				ownerEpoch++;
				actionBusy = false;
				importOpen = false;
				accountOpen = false;
				manualOpen = false;
				editTarget = null;
				deleteTarget = null;
				submitOpen = false;
				selectedIds = [];
				loaded = false;
				candidates = [];
				void loadCandidateList(campaignId);
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		campaignRequest.abort();
		templateRequest.abort();
		candidateRequest.abort();
	});
	function current(epoch: number, key: string) {
		return !disposed && canReadCandidates && epoch === ownerEpoch && context === key;
	}
	function applyCampaign(v: Awaited<typeof initialCampaign>, revision: number) {
		if (!campaignRequest.isCurrent(revision)) return;
		campaignLoading = false;
		if (!v.ok) {
			campaignError = v.error;
			return;
		}
		if (v.data.ownerKey !== context) return;
		campaign = v.data.record;
		campaignError = '';
	}
	function applyTemplates(v: Awaited<typeof initialTemplates>, revision: number) {
		if (!templateRequest.isCurrent(revision)) return;
		templatesLoading = false;
		if (!v.ok) {
			templatesError = v.error;
			return;
		}
		if (v.data.ownerKey !== context) return;
		templates = v.data.record ?? [];
		templatesError = '';
	}
	function applyCandidates(v: Awaited<typeof initialCandidates>, revision: number) {
		if (!candidateRequest.isCurrent(revision) || (v.ok && v.data.filterKey !== candidateFilterKey))
			return;
		loading = false;
		tableLoading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== context) return;
		candidates = applyExternalConfirmationIssues(v.data.record?.items ?? []);
		summary = v.data.record?.summary ?? { ...emptySummary };
		loaded = true;
		error = '';
		selectedIds = selectedIds.filter((id) => candidates.some((c) => c.id === id));
	}
	async function retryCampaign() {
		if (disposed || !canReadCandidates) return;
		const ownerKey = context,
			t = campaignRequest.begin();
		campaignLoading = true;
		campaignError = '';
		applyCampaign(
			await captureRouteLoad(
				getCertificateCampaign(campaignId, { signal: t.signal }).then((record) => ({
					ownerKey,
					record
				})),
				'โหลดกิจกรรมไม่สำเร็จ'
			),
			t.revision
		);
	}
	async function retryTemplates() {
		if (disposed || !canReadCandidates) return;
		const ownerKey = context,
			t = templateRequest.begin();
		templatesLoading = true;
		templatesError = '';
		applyTemplates(
			await captureRouteLoad(
				listCertificateTemplates(campaignId, { signal: t.signal }).then((record) => ({
					ownerKey,
					record
				})),
				'โหลดแบบไม่สำเร็จ'
			),
			t.revision
		);
	}
	function commitFilters() {
		if (actionBusy) return;
		const url = new URL(currentUrl);
		if (statusFilter === 'all') url.searchParams.delete('status');
		else url.searchParams.set('status', statusFilter);
		if (templateFilter) url.searchParams.set('templateId', templateFilter);
		else url.searchParams.delete('templateId');
		if (searchQuery) url.searchParams.set('search', searchQuery);
		else url.searchParams.delete('search');
		pushState(resolve(`${url.pathname}${url.search}` as '/staff/certificates'), {
			...page.state,
			certificateCandidateUrl: url.href
		});
	}
	function changeTemplateFilter(value: string) {
		templateFilter = value === ALL_TEMPLATES_VALUE ? '' : value;
		commitFilters();
	}

	const canPrepareCandidates = $derived(
		canUpdate &&
			loaded &&
			!templatesLoading &&
			!templatesError &&
			campaign?.capabilities.canPrepareCandidates === true
	);
	const canCreateCandidates = $derived(canPrepareCandidates);
	const canManageCandidates = $derived(canPrepareCandidates);
	const canDeleteCandidates = $derived(canManageCandidates);
	const canSubmitCandidates = $derived(canSubmit && campaign?.capabilities.canSubmit === true);
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

	async function loadCandidateList(targetCampaignId: string) {
		if (disposed || !canReadCandidates || targetCampaignId !== campaignId) return;
		const ownerKey = context,
			filterKey = candidateFilterKey,
			t = candidateRequest.begin();
		loading = true;
		tableLoading = loaded;
		error = '';
		applyCandidates(
			await captureRouteLoad(
				listCertificateCandidates(
					targetCampaignId,
					{
						status: statusFilter === 'all' ? undefined : statusFilter,
						templateId: templateFilter || undefined,
						search: searchQuery || undefined
					},
					{ signal: t.signal }
				).then((record) => ({ ownerKey, filterKey, record })),
				'โหลดรายชื่อผู้รับไม่สำเร็จ'
			),
			t.revision
		);
	}
	function applySearch() {
		searchQuery = searchDraft.trim();
		commitFilters();
	}
	function setStatusFilter(status: StatusFilter) {
		if (status === statusFilter) return;
		statusFilter = status;
		commitFilters();
	}
	function patchCandidates(items: CertificateCandidateDetail[]) {
		candidateRequest.abort();
		loading = false;
		tableLoading = false;
		error = '';
		const next = { ...summary };
		const count = (item: CertificateCandidateDetail, delta: number) => {
			if (item.deletedAt) return;
			next.totalCount += delta;
			if (item.validationStatus === 'ready') next.readyCount += delta;
			else if (item.validationStatus === 'needs_review') next.reviewCount += delta;
			else next.invalidCount += delta;
		};
		for (const item of items) {
			const previous = candidates.find((c) => c.id === item.id);
			if (previous) count(previous, -1);
			count(item, 1);
		}
		summary = next;
		const ids = new Set(items.map((c) => c.id));
		const matches = (c: CertificateCandidateDetail) =>
			!c.deletedAt &&
			(statusFilter === 'all' || c.validationStatus === statusFilter) &&
			(!templateFilter || c.templateId === templateFilter) &&
			(!searchQuery ||
				[c.importedFirstName, c.importedLastName, c.activityItem ?? '', c.awardOrRole ?? ''].some(
					(x) => x.toLocaleLowerCase('th').includes(searchQuery.toLocaleLowerCase('th'))
				));
		candidates = [...candidates.filter((c) => !ids.has(c.id)), ...items.filter(matches)].sort(
			(a, b) => a.createdAt.localeCompare(b.createdAt) || a.id.localeCompare(b.id)
		);
		loaded = true;
		selectedIds = selectedIds.filter((id) => candidates.some((c) => c.id === id));
	}

	async function applyBulk(payload: CertificateCandidateBulkRequest, successMessage: string) {
		if (!canManageCandidates || actionBusy || payload.candidateIds.length === 0) return;
		const epoch = ownerEpoch,
			key = context,
			targetCampaignId = campaignId;
		actionBusy = true;
		try {
			const result = await bulkUpdateCertificateCandidates(targetCampaignId, payload);
			if (!current(epoch, key) || !canManageCandidates) return;
			patchCandidates(result.candidates);
			clearExternalConfirmationIssues(payload.candidateIds);
			selectedIds = [];
			toast.success(successMessage);
		} catch (bulkError) {
			if (!current(epoch, key)) return;
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
			toast.error(bulkError instanceof Error ? bulkError.message : 'ปรับปรุงรายชื่อไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleImport(parsed: ParsedCertificateImport) {
		if (!canCreateCandidates || actionBusy) return;
		const epoch = ownerEpoch,
			key = context,
			targetCampaignId = campaignId;
		actionBusy = true;
		try {
			const result = await importCertificateCandidates(targetCampaignId, parsed);
			if (!current(epoch, key) || !canCreateCandidates) return;
			patchCandidates(result.candidates);
			importOpen = false;
			toast.success(`นำเข้า ${result.batch.rowCount.toLocaleString('th-TH')} รายการแล้ว`);
		} catch (importError) {
			if (!current(epoch, key)) return;
			toast.error(importError instanceof Error ? importError.message : 'นำเข้ารายชื่อไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleAccountCreate(payload: CreateAccountCertificateCandidateRequest) {
		if (!canCreateCandidates || actionBusy) return;
		const epoch = ownerEpoch,
			key = context,
			targetCampaignId = campaignId;
		actionBusy = true;
		try {
			const result = await createAccountCertificateCandidate(targetCampaignId, payload);
			if (!current(epoch, key) || !canCreateCandidates) return;
			patchCandidates(result.candidates);
			accountOpen = false;
			toast.success('เพิ่มผู้รับจากบัญชีแล้ว');
		} catch (createError) {
			if (!current(epoch, key)) return;
			toast.error(
				createError instanceof Error ? createError.message : 'เพิ่มผู้รับจากบัญชีไม่สำเร็จ'
			);
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleManualCreate(payload: CreateManualExternalCandidateRequest) {
		if (!canCreateCandidates || actionBusy) return;
		const epoch = ownerEpoch,
			key = context,
			targetCampaignId = campaignId;
		actionBusy = true;
		try {
			const result = await createManualCertificateCandidate(targetCampaignId, payload);
			if (!current(epoch, key) || !canCreateCandidates) return;
			patchCandidates(result.candidates);
			manualOpen = false;
			toast.success('เพิ่มบุคคลภายนอกแล้ว');
		} catch (createError) {
			if (!current(epoch, key)) return;
			toast.error(createError instanceof Error ? createError.message : 'เพิ่มบุคคลภายนอกไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleEdit(payload: UpdateCertificateCandidateRequest) {
		if (!editTarget || !canManageCandidates || actionBusy) return;
		const epoch = ownerEpoch,
			key = context;
		actionBusy = true;
		try {
			const candidateId = editTarget.id;
			const updated = await updateCertificateCandidate(candidateId, payload);
			if (!current(epoch, key) || !canManageCandidates) return;
			patchCandidates([updated]);
			clearExternalConfirmationIssues([candidateId]);
			editTarget = null;
			toast.success('บันทึกรายชื่อแล้ว');
		} catch (updateError) {
			if (!current(epoch, key)) return;
			toast.error(updateError instanceof Error ? updateError.message : 'บันทึกรายชื่อไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleDelete() {
		if (!deleteTarget || !canDeleteCandidates || actionBusy) return;
		const target = deleteTarget;
		const epoch = ownerEpoch,
			key = context;
		actionBusy = true;
		try {
			const updated = await deleteCertificateCandidate(target.id);
			if (!current(epoch, key) || !canDeleteCandidates) return;
			patchCandidates([updated]);
			clearExternalConfirmationIssues([target.id]);
			deleteTarget = null;
			selectedIds = selectedIds.filter((id) => id !== target.id);
			toast.success('ลบรายชื่อแล้ว');
		} catch (deleteError) {
			if (!current(epoch, key)) return;
			toast.error(deleteError instanceof Error ? deleteError.message : 'ลบรายชื่อไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) actionBusy = false;
		}
	}

	async function handleSubmitRequest(candidateIds: string[]) {
		if (!canSubmitSelection || actionBusy) return;
		const epoch = ownerEpoch,
			key = context,
			targetCampaignId = campaignId;
		actionBusy = true;
		submitError = '';
		lockedRequestId = null;
		try {
			await submitCertificateIssueRequest(targetCampaignId, { candidateIds });
			if (!current(epoch, key) || !canSubmitCandidates) return;
			selectedIds = [];
			submitOpen = false;
			await Promise.all([retryCampaign(), retryTemplates(), loadCandidateList(targetCampaignId)]);
			if (!current(epoch, key)) return;
			toast.success('ส่งคำขอออกเกียรติบัตรแล้ว');
		} catch (submitFailure) {
			if (!current(epoch, key)) return;
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
			if (current(epoch, key)) actionBusy = false;
		}
	}
</script>

<PageShell
	title="ตรวจรายชื่อผู้รับ"
	description="นำเข้ารายชื่อ เชื่อมบัญชี แก้คำเตือน และกำหนดแบบให้พร้อมก่อนส่งคำขอออกเกียรติบัตร"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<UsersRound class="size-4" />
			{loaded
				? `${summary.totalCount.toLocaleString('th-TH')} รายการ`
				: 'กำลังโหลดรายชื่อ'}{campaign ? ` · ${campaign.name}` : ''}
		</div>
	{/snippet}

	{#snippet actions()}
		{#if canReadCandidates}<Button variant="outline" onclick={() => loadCandidateList(campaignId)}
				>โหลดรายชื่อใหม่</Button
			>{/if}
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
	{:else}
		{#if campaignLoading}<p role="status">กำลังโหลดกิจกรรม</p>{/if}
		{#if campaignError}<PageState
				variant="error"
				title="โหลดกิจกรรมไม่สำเร็จ"
				description={campaignError}
				actionLabel="ลองกิจกรรมอีกครั้ง"
				onaction={retryCampaign}
			/>{/if}
		{#if templatesLoading}<p role="status">กำลังโหลดแบบเกียรติบัตร</p>{/if}
		{#if templatesError}<PageState
				variant="error"
				title="โหลดแบบไม่สำเร็จ"
				description={templatesError}
				actionLabel="ลองแบบอีกครั้ง"
				onaction={retryTemplates}
			/>{/if}
		{#if loading && !loaded}<div role="status" aria-label="กำลังโหลดรายชื่อผู้รับ">
				<PageSkeleton variant="table" />
			</div>{/if}
		{#if error}
			<PageState
				variant="error"
				title="โหลดรายชื่อผู้รับไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => loadCandidateList(campaignId)}
			/>
		{/if}
		{#if loaded}
			<section
				class="overflow-hidden rounded-xl border bg-card shadow-sm"
				aria-label="สรุปความพร้อมของรายชื่อ"
			>
				<div
					class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/25 px-5 py-3"
				>
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
						onclick={() =>
							setStatusFilter(statusFilter === 'needs_review' ? 'all' : 'needs_review')}
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
						<Select.Root
							type="single"
							value={templateFilter || ALL_TEMPLATES_VALUE}
							onValueChange={changeTemplateFilter}
						>
							<Select.Trigger class="w-full">
								{templates.find((template) => template.id === templateFilter)?.name ?? 'ทุกแบบ'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={ALL_TEMPLATES_VALUE}>ทุกแบบ</Select.Item>
								{#each templates as template (template.id)}
									<Select.Item value={template.id}>{template.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
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
								<Select.Root
									type="single"
									value={bulkTemplateId || NO_BULK_TEMPLATE_VALUE}
									onValueChange={(value) =>
										(bulkTemplateId = value === NO_BULK_TEMPLATE_VALUE ? '' : value)}
								>
									<Select.Trigger class="w-full">
										{@const template = bulkCompatibleTemplates.find(
											(item) => item.id === bulkTemplateId
										)}
										{template
											? `${template.name} · ${template.allowedRecipientTypes
													.map((type) => recipientTypeLabels[type])
													.join('/')}`
											: 'เลือกแบบเกียรติบัตร'}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value={NO_BULK_TEMPLATE_VALUE}>เลือกแบบเกียรติบัตร</Select.Item>
										{#each bulkCompatibleTemplates as template (template.id)}
											<Select.Item value={template.id}>
												{template.name} · {template.allowedRecipientTypes
													.map((type) => recipientTypeLabels[type])
													.join('/')}
											</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
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
			{/if}
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

{#if importOpen && canCreateCandidates}
	<CertificateImportDialog
		open={importOpen}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (importOpen = open)}
		onimport={handleImport}
	/>
{/if}

{#if accountOpen && canCreateCandidates}
	<CertificateAccountSearchDialog
		open={accountOpen}
		{campaignId}
		{templates}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (accountOpen = open)}
		oncreate={handleAccountCreate}
	/>
{/if}

{#if manualOpen && canCreateCandidates}
	<CertificateManualExternalDialog
		open={manualOpen}
		{templates}
		busy={actionBusy}
		onopenchange={(open) => !actionBusy && (manualOpen = open)}
		oncreate={handleManualCreate}
	/>
{/if}

{#if editTarget && canManageCandidates}
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

{#if submitOpen && campaign && canSubmitCandidates}
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
