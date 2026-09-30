<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import {
		changeCertificateCampaignStatus,
		getCertificateCampaign,
		listCertificateOwnerOptions,
		updateCertificateCampaign,
		type CertificateCampaignDetail,
		type CertificateCampaignStatus
	} from '$lib/api/certificates';
	import {
		lookupAcademicYears,
		type AcademicYearLookupItem,
		type OrganizationUnitLookupItem
	} from '$lib/api/lookup';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateCampaignForm, {
		type CertificateCampaignFormValue
	} from '$lib/components/certificates/CertificateCampaignForm.svelte';
	import CertificateCampaignPurgeDialog from '$lib/components/certificates/CertificateCampaignPurgeDialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import {
		Archive,
		Award,
		Building2,
		CalendarDays,
		FileBadge2,
		LockKeyhole,
		Pencil,
		RotateCcw,
		Trash2,
		UsersRound
	} from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	type CertificateAcademicYearOption = Omit<AcademicYearLookupItem, 'status'> & {
		status?: AcademicYearLookupItem['status'];
	};

	const campaignId = $derived(page.params.campaignId ?? '');
	const canReadCampaign = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canCreateOrganizationCampaign = $derived(
		$can.has(PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT)
	);
	const canCreateSchoolCampaign = $derived($can.has(PERMISSIONS.CERTIFICATE_CREATE_SCHOOL));
	const canCreateCampaign = $derived(canCreateOrganizationCampaign || canCreateSchoolCampaign);

	let { data }: PageProps = $props();
	let campaign: CertificateCampaignDetail | null = $state.raw(null),
		editingCampaign: CertificateCampaignDetail | null = $state.raw(null);
	let academicYears: CertificateAcademicYearOption[] = $state.raw([]),
		ownerOptions: OrganizationUnitLookupItem[] = $state.raw([]);
	let loading = $state(true),
		error = $state(''),
		editOpen = $state(false),
		saving = $state(false),
		deleteOpen = $state(false);
	let yearsLoading = $state(false),
		ownersLoading = $state(false),
		yearsReady = $state(false),
		ownersReady = $state(false),
		yearsError = $state(''),
		ownersError = $state('');
	let statusBusy = $state<CertificateCampaignStatus | null>(null);
	let savingDraft = $state(-1);
	const identity = $derived.by(() => {
		void $authStore.user;
		void $can;
		return appIdentityKey();
	});
	const context = $derived(`${identity}|${campaignId}`);
	const canEdit = $derived.by(
		() =>
			Boolean(campaign?.capabilities.canUpdate) &&
			$can.hasAny(
				PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
				PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
			)
	);
	const canChangeStatus = $derived.by(
		() =>
			Boolean(campaign?.capabilities.canChangeStatus) &&
			$can.hasAny(
				PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
				PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
			)
	);
	const canDelete = $derived.by(
		() =>
			Boolean(campaign?.capabilities.canDelete) &&
			$can.hasAny(
				PERMISSIONS.CERTIFICATE_DELETE_ORGANIZATION_UNIT,
				PERMISSIONS.CERTIFICATE_DELETE_SCHOOL
			)
	);
	const request = new LatestRequest(),
		yearsRequest = new LatestRequest(),
		ownersRequest = new LatestRequest();
	let owner = $state(''),
		ownerEpoch = $state(0),
		editEpoch = $state(0),
		disposed = false;
	let consumed: typeof data.campaign | null = null;
	$effect.pre(() => {
		const key = context,
			source = data.campaign,
			allowed = canReadCampaign;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				request.abort();
				campaign = null;
				loading = allowed;
				error = '';
				closeEdit();
				deleteOpen = false;
				saving = false;
				statusBusy = null;
			}
			if (!allowed || consumed === source) return;
			consumed = source;
			const t = request.begin();
			loading = true;
			error = '';
			void source.then((r) => applyCampaign(r, t.revision));
		});
	});
	$effect.pre(() => {
		const opened = editOpen,
			allowed = canEdit;
		if (!opened || !allowed) untrack(closeEdit);
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		request.abort();
		closeEdit();
	});
	function applyCampaign(r: Awaited<typeof data.campaign>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (!r.ok) {
			error = r.error;
			return;
		}
		if (r.data.identityKey !== identity || r.data.campaignId !== campaignId) return;
		campaign = r.data.record;
	}
	function currentOwner(epoch: number, key: string) {
		return !disposed && epoch === ownerEpoch && key === context && canReadCampaign;
	}
	function currentEdit(epoch: number, key: string, draft: number) {
		return currentOwner(epoch, key) && canEdit && editOpen && draft === editEpoch;
	}
	function closeEdit() {
		editOpen = false;
		editEpoch++;
		yearsRequest.abort();
		ownersRequest.abort();
		editingCampaign = null;
		academicYears = [];
		ownerOptions = [];
		yearsReady = ownersReady = yearsLoading = ownersLoading = false;
		yearsError = ownersError = '';
	}
	const statusLabels: Record<CertificateCampaignStatus, string> = {
		draft: 'ฉบับร่าง',
		active: 'กำลังออก',
		closed: 'ปิดกิจกรรม',
		archived: 'เก็บถาวร',
		purging: 'กำลังลบ'
	};

	const statusClasses: Record<CertificateCampaignStatus, string> = {
		draft: 'border-slate-200 bg-slate-50 text-slate-700',
		active: 'border-emerald-200 bg-emerald-50 text-emerald-700',
		closed: 'border-amber-200 bg-amber-50 text-amber-700',
		archived: 'border-zinc-200 bg-zinc-100 text-zinc-600',
		purging: 'border-red-200 bg-red-50 text-red-700'
	};

	async function loadCampaign() {
		if (disposed || !canReadCampaign) return;
		const identityKey = identity,
			selectedId = campaignId,
			t = request.begin();
		loading = true;
		error = '';
		applyCampaign(
			await captureRouteLoad(
				getCertificateCampaign(selectedId, { signal: t.signal }).then((record) => ({
					identityKey,
					campaignId: selectedId,
					record
				})),
				'โหลดกิจกรรมไม่สำเร็จ'
			),
			t.revision
		);
	}
	function currentAcademicYear(
		selected: CertificateCampaignDetail
	): CertificateAcademicYearOption[] {
		return [
			{
				id: selected.academicYearId,
				name: selected.academicYearName,
				year: selected.academicYearValue
			}
		];
	}
	function currentOwnerOptions(selected: CertificateCampaignDetail): OrganizationUnitLookupItem[] {
		if (!selected.ownerOrganizationUnitId || !selected.ownerOrganizationUnitCode) return [];
		return [
			{
				id: selected.ownerOrganizationUnitId,
				code: selected.ownerOrganizationUnitCode,
				name: selected.ownerOrganizationUnitName ?? selected.ownerOrganizationUnitCode,
				display_order: 0,
				is_active: true
			}
		];
	}
	function openEdit() {
		if (!campaign || !canEdit || disposed) return;
		closeEdit();
		editingCampaign = structuredClone(campaign);
		editOpen = true;
		academicYears = currentAcademicYear(editingCampaign);
		ownerOptions = currentOwnerOptions(editingCampaign);
		if (editingCampaign.activitySequence !== null) {
			yearsReady = ownersReady = true;
			return;
		}
		void loadEditYears();
		if (canCreateCampaign) void loadEditOwners();
		else ownersReady = true;
	}
	async function loadEditYears() {
		if (!editingCampaign || !editOpen || !canEdit) return;
		const selected = editingCampaign,
			epoch = ownerEpoch,
			key = context,
			draft = editEpoch,
			t = yearsRequest.begin();
		yearsLoading = true;
		yearsError = '';
		const r = await captureRouteLoad(
			lookupAcademicYears({ activeOnly: false }, { signal: t.signal }),
			'โหลดปีการศึกษาไม่สำเร็จ'
		);
		if (!yearsRequest.isCurrent(t.revision) || !currentEdit(epoch, key, draft)) return;
		yearsLoading = false;
		if (!r.ok) {
			yearsError = r.error;
			return;
		}
		academicYears = r.data.length ? r.data : currentAcademicYear(selected);
		yearsReady = true;
	}
	async function loadEditOwners() {
		if (!editingCampaign || !editOpen || !canEdit || !canCreateCampaign) return;
		const epoch = ownerEpoch,
			key = context,
			draft = editEpoch,
			t = ownersRequest.begin();
		ownersLoading = true;
		ownersError = '';
		const r = await captureRouteLoad(
			listCertificateOwnerOptions({ signal: t.signal }),
			'โหลดหน่วยงานเจ้าของไม่สำเร็จ'
		);
		if (!ownersRequest.isCurrent(t.revision) || !currentEdit(epoch, key, draft)) return;
		ownersLoading = false;
		if (!r.ok) {
			ownersError = r.error;
			return;
		}
		ownerOptions = r.data;
		ownersReady = true;
	}
	async function handleUpdate(value: CertificateCampaignFormValue) {
		if (
			!editingCampaign ||
			!canEdit ||
			saving ||
			statusBusy ||
			!yearsReady ||
			!ownersReady ||
			yearsError ||
			ownersError
		)
			return;
		const selected = editingCampaign,
			epoch = ownerEpoch,
			key = context,
			draft = editEpoch;
		savingDraft = draft;
		saving = true;
		try {
			const saved = await updateCertificateCampaign(selected.id, {
				expectedUpdatedAt: selected.updatedAt,
				academicYearId: value.academicYearId,
				ownerOrganizationUnitId: { value: value.ownerOrganizationUnitId ?? null },
				name: value.name,
				eventDate: value.eventDate,
				confirmAffectsIssuedCertificates: value.confirmAffectsIssuedCertificates
			});
			if (!currentOwner(epoch, key) || !canEdit || saved.id !== campaignId) return;
			const ownsDraft = currentEdit(epoch, key, draft);
			request.abort();
			loading = false;
			error = '';
			campaign = saved;
			if (ownsDraft) {
				closeEdit();
				toast.success('บันทึกข้อมูลกิจกรรมแล้ว');
			}
		} catch (updateError) {
			if (currentEdit(epoch, key, draft))
				toast.error(updateError instanceof Error ? updateError.message : 'บันทึกกิจกรรมไม่สำเร็จ');
		} finally {
			if (currentOwner(epoch, key)) saving = false;
		}
	}
	async function handleStatus(nextStatus: CertificateCampaignStatus) {
		if (!campaign || !canChangeStatus || statusBusy || saving) return;
		const selected = campaign,
			epoch = ownerEpoch,
			key = context;
		statusBusy = nextStatus;
		try {
			const saved = await changeCertificateCampaignStatus(selected.id, {
				expectedUpdatedAt: selected.updatedAt,
				status: nextStatus
			});
			if (!currentOwner(epoch, key) || !canChangeStatus || saved.id !== campaignId) return;
			request.abort();
			loading = false;
			error = '';
			campaign = saved;
			toast.success(`เปลี่ยนสถานะเป็น “${statusLabels[nextStatus]}” แล้ว`);
		} catch (statusError) {
			if (currentOwner(epoch, key) && canChangeStatus)
				toast.error(statusError instanceof Error ? statusError.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		} finally {
			if (currentOwner(epoch, key)) statusBusy = null;
		}
	}
	function handlePurged(epoch: number, key: string) {
		if (!currentOwner(epoch, key) || !canDelete) return;
		deleteOpen = false;
		toast.success('ลบกิจกรรมและไฟล์ทั้งหมดแล้ว');
		void goto(resolve('/staff/certificates'));
	}
	function formatDate(value: string): string {
		return new Date(`${value}T00:00:00`).toLocaleDateString('th-TH', {
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
	}
</script>

<PageShell
	title={campaign?.name ?? 'ภาพรวมชุดออกเกียรติบัตร'}
	description="ข้อมูลร่วม สถานะ และความคืบหน้าของกิจกรรม"
	backHref="/staff/certificates"
>
	{#snippet meta()}
		{#if campaign}
			<div class="flex flex-wrap items-center gap-2">
				<Badge variant="outline" class={statusClasses[campaign.status]}>
					{statusLabels[campaign.status]}
				</Badge>
				{#if campaign.activitySequence !== null}
					<Badge variant="secondary">กิจกรรมลำดับ {campaign.activitySequence}</Badge>
				{/if}
			</div>
		{/if}
	{/snippet}

	{#snippet actions()}
		{#if canEdit && campaign}
			<Button variant="outline" onclick={openEdit}>
				<Pencil class="size-4" />
				แก้ข้อมูล
			</Button>
		{/if}
		{#if canChangeStatus && campaign}
			{#if campaign.status === 'active'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'closed'}
					disabled={statusBusy !== null || saving}
					onclick={() => handleStatus('closed')}
				>
					<LockKeyhole class="size-4" /> ปิดกิจกรรม
				</LoadingButton>
			{/if}
			{#if campaign.status === 'closed' || campaign.status === 'archived'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'active'}
					disabled={statusBusy !== null || saving}
					onclick={() => handleStatus('active')}
				>
					<RotateCcw class="size-4" /> เปิดกิจกรรมอีกครั้ง
				</LoadingButton>
			{/if}
			{#if campaign.status === 'active' || campaign.status === 'closed'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'archived'}
					disabled={statusBusy !== null || saving}
					onclick={() => handleStatus('archived')}
				>
					<Archive class="size-4" /> เก็บถาวร
				</LoadingButton>
			{/if}
		{/if}
	{/snippet}

	{#if !canReadCampaign}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูกิจกรรมนี้"
			description="ระบบตรวจทั้งสิทธิ์และหน่วยงานเจ้าของกิจกรรมจาก backend"
		/>
	{:else if loading && !campaign}
		<div role="status" aria-label="กำลังโหลดกิจกรรมเกียรติบัตร">
			<PageSkeleton variant="detail" />
		</div>
	{:else}
		{#if loading && campaign}<p role="status">กำลังอัปเดตกิจกรรม</p>{/if}
		{#if error}
			<PageState
				variant="error"
				title="โหลดกิจกรรมไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadCampaign}
			/>
		{/if}
		{#if campaign}
			{#if campaign.hasOpenIssueRequest}
				<div
					class="flex items-start gap-3 rounded-xl border border-blue-200 bg-blue-50 p-4 text-blue-950"
				>
					<LockKeyhole class="mt-0.5 size-5 shrink-0" />
					<div>
						<p class="font-medium">มีคำขอออกเกียรติบัตรที่กำลังตรวจ</p>
						<p class="mt-1 text-sm text-blue-800">
							ข้อมูลร่วมและรายการในคำขอถูกล็อกจนกว่าจะออก ส่งกลับ หรือถอนคำขอ
						</p>
					</div>
				</div>
			{/if}

			<Card.Root class="overflow-hidden py-0">
				<div class="grid lg:grid-cols-[minmax(0,1.4fr)_minmax(18rem,0.6fr)]">
					<Card.Content class="space-y-6 p-5 lg:p-6">
						<div>
							<p class="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
								ข้อมูลกิจกรรม
							</p>
							<h2 class="mt-2 text-xl font-semibold text-foreground">{campaign.name}</h2>
						</div>

						<dl class="grid gap-5 sm:grid-cols-2">
							<div class="space-y-1">
								<dt class="flex items-center gap-2 text-sm text-muted-foreground">
									<CalendarDays class="size-4" /> ปีการศึกษาและวันที่จัด
								</dt>
								<dd class="font-medium">
									{campaign.academicYearName} · {formatDate(campaign.eventDate)}
								</dd>
							</div>
							<div class="space-y-1">
								<dt class="flex items-center gap-2 text-sm text-muted-foreground">
									<Building2 class="size-4" /> หน่วยงานเจ้าของ
								</dt>
								<dd class="font-medium">
									{campaign.ownerOrganizationUnitName ?? 'กิจกรรมระดับโรงเรียน'}
								</dd>
							</div>
						</dl>
					</Card.Content>

					<div
						class="grid grid-cols-3 border-t bg-muted/20 lg:grid-cols-1 lg:border-l lg:border-t-0"
					>
						<div class="p-4 text-center lg:flex lg:items-center lg:justify-between lg:text-left">
							<span class="inline-flex items-center gap-2 text-xs text-muted-foreground">
								<FileBadge2 class="size-4" /> แบบ
							</span>
							<strong class="mt-1 block text-xl tabular-nums lg:mt-0"
								>{campaign.templateCount}</strong
							>
						</div>
						<div
							class="border-l p-4 text-center lg:flex lg:items-center lg:justify-between lg:border-l-0 lg:border-t lg:text-left"
						>
							<span class="inline-flex items-center gap-2 text-xs text-muted-foreground">
								<UsersRound class="size-4" /> รายชื่อ
							</span>
							<strong class="mt-1 block text-xl tabular-nums lg:mt-0"
								>{campaign.candidateCount}</strong
							>
						</div>
						<div
							class="border-l p-4 text-center lg:flex lg:items-center lg:justify-between lg:border-l-0 lg:border-t lg:text-left"
						>
							<span class="inline-flex items-center gap-2 text-xs text-muted-foreground">
								<Award class="size-4" /> ออกแล้ว
							</span>
							<strong class="mt-1 block text-xl tabular-nums lg:mt-0">
								{campaign.issuedCertificateCount}
							</strong>
						</div>
					</div>
				</div>
			</Card.Root>

			<div class="rounded-xl border border-dashed p-4">
				<div>
					<p class="font-medium">ลำดับเลขเกียรติบัตร</p>
					<p class="mt-1 text-sm text-muted-foreground">
						{campaign.activitySequence === null
							? 'ยังไม่จองเลขกิจกรรม ระบบจะจองเมื่อออกใบแรก'
							: `กิจกรรมลำดับ ${campaign.activitySequence} · ใบถัดไปลำดับ ${campaign.nextCertificateSequence}`}
					</p>
				</div>
			</div>

			{#if canDelete}
				<Card.Root class="gap-0 border-destructive/30 bg-destructive/[0.025] py-0">
					<Card.Content
						class="flex flex-col gap-4 p-5 sm:flex-row sm:items-center sm:justify-between"
					>
						<div class="flex items-start gap-3">
							<div
								class="flex size-10 shrink-0 items-center justify-center rounded-full bg-destructive/10 text-destructive"
							>
								<Trash2 class="size-4" />
							</div>
							<div>
								<p class="font-semibold text-destructive">พื้นที่อันตราย</p>
								<p class="mt-1 max-w-2xl text-sm text-muted-foreground">
									ลบกิจกรรม แม่แบบ รายชื่อ คำขอ เกียรติบัตร และไฟล์ทั้งหมดแบบถาวร
									ระบบจะแสดงจำนวนให้ตรวจอีกครั้งก่อนยืนยัน
								</p>
							</div>
						</div>
						<Button variant="destructive" class="shrink-0" onclick={() => (deleteOpen = true)}>
							<Trash2 class="size-4" /> ลบกิจกรรมถาวร
						</Button>
					</Card.Content>
				</Card.Root>
			{/if}
		{/if}
	{/if}
</PageShell>

{#if canEdit && campaign}
	<Dialog.Root bind:open={editOpen}>
		<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
			<Dialog.Header
				><Dialog.Title>แก้ข้อมูลกิจกรรม</Dialog.Title><Dialog.Description
					>หลังออกใบแรกจะเปลี่ยนปีการศึกษาและหน่วยงานเจ้าของไม่ได้</Dialog.Description
				></Dialog.Header
			>
			{#if editingCampaign && editOpen}
				<section data-testid="edit-campaign-years" aria-busy={yearsLoading}>
					{#if yearsError}<PageState
							variant="error"
							title="โหลดปีการศึกษาไม่สำเร็จ"
							description={yearsError}
							actionLabel="ลองโหลดปีการศึกษาอีกครั้ง"
							onaction={loadEditYears}
						/>{/if}
					{#if yearsLoading}<div role="status" aria-label="กำลังโหลดปีการศึกษา">
							<PageSkeleton variant="form" rows={1} />
						</div>{/if}
				</section>
				<section data-testid="edit-campaign-owners" aria-busy={ownersLoading}>
					{#if ownersError}<PageState
							variant="error"
							title="โหลดหน่วยงานเจ้าของไม่สำเร็จ"
							description={ownersError}
							actionLabel="ลองโหลดหน่วยงานอีกครั้ง"
							onaction={loadEditOwners}
						/>{/if}
					{#if ownersLoading}<div role="status" aria-label="กำลังโหลดหน่วยงานเจ้าของ">
							<PageSkeleton variant="form" rows={1} />
						</div>{/if}
				</section>
				{#key editEpoch}
					<CertificateCampaignForm
						{academicYears}
						{ownerOptions}
						campaign={editingCampaign}
						allowSchoolOwner={canCreateSchoolCampaign}
						allowOwnerChange={canCreateCampaign}
						saving={saving && savingDraft === editEpoch}
						yearsReady={yearsReady && !yearsError}
						ownersReady={ownersReady && !ownersError}
						disabled={statusBusy !== null || saving}
						onsubmit={handleUpdate}
						oncancel={closeEdit}
					/>
				{/key}
			{/if}
		</Dialog.Content>
	</Dialog.Root>
{/if}
{#if deleteOpen && campaign && canDelete}
	{#key owner}
		{@const purgeEpoch = ownerEpoch}
		{@const purgeKey = owner}
		<CertificateCampaignPurgeDialog
			open={deleteOpen}
			campaignId={campaign.id}
			campaignName={campaign.name}
			onopenchange={(open) => {
				if (currentOwner(purgeEpoch, purgeKey)) deleteOpen = open;
			}}
			oncompleted={() => handlePurged(purgeEpoch, purgeKey)}
		/>
	{/key}
{/if}
