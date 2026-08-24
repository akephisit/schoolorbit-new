<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
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
	} from 'lucide-svelte';
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

	let campaign: CertificateCampaignDetail | null = $state.raw(null);
	let academicYears: CertificateAcademicYearOption[] = $state.raw([]);
	let ownerOptions: OrganizationUnitLookupItem[] = $state.raw([]);
	let loading = $state(true);
	let error = $state('');
	let editOpen = $state(false);
	let editReferencesLoaded = $state(false);
	let loadingEditReferences = $state(false);
	let saving = $state(false);
	let statusBusy = $state<CertificateCampaignStatus | null>(null);
	let deleteOpen = $state(false);

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
		if (!canReadCampaign) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			campaign = await getCertificateCampaign(campaignId);
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดกิจกรรมได้';
		} finally {
			loading = false;
		}
	}

	function currentAcademicYear(): CertificateAcademicYearOption[] {
		if (!campaign) return [];
		return [
			{
				id: campaign.academicYearId,
				name: campaign.academicYearName,
				year: campaign.academicYearValue
			}
		];
	}

	function currentOwner(): OrganizationUnitLookupItem[] {
		if (!campaign?.ownerOrganizationUnitId || !campaign.ownerOrganizationUnitCode) return [];
		return [
			{
				id: campaign.ownerOrganizationUnitId,
				code: campaign.ownerOrganizationUnitCode,
				name: campaign.ownerOrganizationUnitName ?? campaign.ownerOrganizationUnitCode,
				display_order: 0,
				is_active: true
			}
		];
	}

	async function openEdit() {
		if (!campaign?.capabilities.canUpdate) return;
		loadingEditReferences = true;
		try {
			if (!editReferencesLoaded) {
				academicYears = await lookupAcademicYears({ activeOnly: false });
				if (canCreateCampaign) {
					ownerOptions = await listCertificateOwnerOptions();
				} else {
					ownerOptions = currentOwner();
				}
				if (academicYears.length === 0) academicYears = currentAcademicYear();
				editReferencesLoaded = true;
			}
			editOpen = true;
		} catch (loadError) {
			toast.error(loadError instanceof Error ? loadError.message : 'โหลดแบบฟอร์มแก้ไขไม่สำเร็จ');
		} finally {
			loadingEditReferences = false;
		}
	}

	async function handleUpdate(value: CertificateCampaignFormValue) {
		if (!campaign?.capabilities.canUpdate || saving) return;
		saving = true;
		try {
			campaign = await updateCertificateCampaign(campaign.id, {
				expectedUpdatedAt: campaign.updatedAt,
				academicYearId: value.academicYearId,
				ownerOrganizationUnitId: { value: value.ownerOrganizationUnitId ?? null },
				name: value.name,
				eventDate: value.eventDate,
				confirmAffectsIssuedCertificates: value.confirmAffectsIssuedCertificates
			});
			editOpen = false;
			toast.success('บันทึกข้อมูลกิจกรรมแล้ว');
		} catch (updateError) {
			toast.error(updateError instanceof Error ? updateError.message : 'บันทึกกิจกรรมไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function handleStatus(nextStatus: CertificateCampaignStatus) {
		if (!campaign?.capabilities.canChangeStatus || statusBusy) return;
		statusBusy = nextStatus;
		try {
			campaign = await changeCertificateCampaignStatus(campaign.id, {
				expectedUpdatedAt: campaign.updatedAt,
				status: nextStatus
			});
			toast.success(`เปลี่ยนสถานะเป็น “${statusLabels[nextStatus]}” แล้ว`);
		} catch (statusError) {
			toast.error(statusError instanceof Error ? statusError.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		} finally {
			statusBusy = null;
		}
	}

	function handlePurged() {
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

	onMount(loadCampaign);
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
		{#if campaign?.capabilities.canUpdate}
			<Button variant="outline" onclick={openEdit} disabled={loadingEditReferences}>
				<Pencil class="size-4" />
				{loadingEditReferences ? 'กำลังโหลด...' : 'แก้ข้อมูล'}
			</Button>
		{/if}
		{#if campaign?.capabilities.canChangeStatus}
			{#if campaign.status === 'active'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'closed'}
					disabled={statusBusy !== null}
					onclick={() => handleStatus('closed')}
				>
					<LockKeyhole class="size-4" /> ปิดกิจกรรม
				</LoadingButton>
			{/if}
			{#if campaign.status === 'closed' || campaign.status === 'archived'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'active'}
					disabled={statusBusy !== null}
					onclick={() => handleStatus('active')}
				>
					<RotateCcw class="size-4" /> เปิดกิจกรรมอีกครั้ง
				</LoadingButton>
			{/if}
			{#if campaign.status === 'active' || campaign.status === 'closed'}
				<LoadingButton
					variant="outline"
					loading={statusBusy === 'archived'}
					disabled={statusBusy !== null}
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
	{:else if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดกิจกรรมไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadCampaign}
		/>
	{:else if campaign}
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

				<div class="grid grid-cols-3 border-t bg-muted/20 lg:grid-cols-1 lg:border-l lg:border-t-0">
					<div class="p-4 text-center lg:flex lg:items-center lg:justify-between lg:text-left">
						<span class="inline-flex items-center gap-2 text-xs text-muted-foreground">
							<FileBadge2 class="size-4" /> แบบ
						</span>
						<strong class="mt-1 block text-xl tabular-nums lg:mt-0">{campaign.templateCount}</strong
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

		{#if campaign.capabilities.canDelete}
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
</PageShell>

{#if campaign?.capabilities.canUpdate}
	<Dialog.Root bind:open={editOpen}>
		<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
			<Dialog.Header>
				<Dialog.Title>แก้ข้อมูลกิจกรรม</Dialog.Title>
				<Dialog.Description>
					หลังออกใบแรกจะเปลี่ยนปีการศึกษาและหน่วยงานเจ้าของไม่ได้
				</Dialog.Description>
			</Dialog.Header>
			{#if editReferencesLoaded}
				{#key campaign.updatedAt}
					<CertificateCampaignForm
						{academicYears}
						{ownerOptions}
						{campaign}
						allowSchoolOwner={canCreateSchoolCampaign}
						allowOwnerChange={canCreateCampaign}
						{saving}
						onsubmit={handleUpdate}
						oncancel={() => (editOpen = false)}
					/>
				{/key}
			{/if}
		</Dialog.Content>
	</Dialog.Root>
{/if}

{#if deleteOpen && campaign?.capabilities.canDelete}
	<CertificateCampaignPurgeDialog
		open={deleteOpen}
		campaignId={campaign.id}
		campaignName={campaign.name}
		onopenchange={(open) => (deleteOpen = open)}
		oncompleted={handlePurged}
	/>
{/if}
