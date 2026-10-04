<script lang="ts">
	import { beforeNavigate } from '$app/navigation';
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { page } from '$app/state';
	import {
		deleteCertificateTemplate,
		getCertificateCampaign,
		getCertificateTemplate,
		listCertificateTemplates,
		type CertificateCampaignDetail,
		type CertificateTemplateDetail
	} from '#lib/api/certificates.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import CertificateTemplateForm from '#lib/components/certificates/CertificateTemplateForm.svelte';
	import CertificateTemplateList from '#lib/components/certificates/CertificateTemplateList.svelte';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { FileBadge2, Plus, Trash2 } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	let campaign = $state.raw<CertificateCampaignDetail | null>(null);

	const campaignId = $derived(page.params.campaignId ?? '');
	const context = $derived(`${identityKey}|${campaignId}`);
	const canReadTemplates = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const hasCreatePermission = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
		)
	);
	const hasUpdatePermission = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
		)
	);
	const canCreateTemplates = $derived(
		hasCreatePermission && hasUpdatePermission && campaign?.capabilities.canManageTemplates === true
	);

	let templates = $state.raw<CertificateTemplateDetail[]>([]);
	let loading = $state(true);
	let error = $state('');
	let formOpen = $state(false);
	let formTemplate = $state.raw<CertificateTemplateDetail | null>(null);
	let deleteTarget = $state.raw<CertificateTemplateDetail | null>(null);
	let deleting = $state(false);
	let formHasPendingUpload = $state(false);
	let listHasPendingUpload = $state(false);
	let disposed = false,
		owner = '',
		ownerEpoch = $state(0);
	let campaignLoading = $state(true),
		campaignError = $state(''),
		loaded = $state(false),
		formEpoch = $state(0);
	const campaignRequest = new LatestRequest(),
		templateRequest = new LatestRequest();
	let consumedCampaign: typeof data.campaign | null = null,
		consumedTemplates: typeof data.templates | null = null;
	$effect.pre(() => {
		const key = context,
			a = data.campaign,
			b = data.templates,
			allowed = canReadTemplates;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				campaignRequest.abort();
				templateRequest.abort();
				campaign = null;
				templates = [];
				loaded = false;
				loading = allowed;
				campaignLoading = allowed;
				campaignError = '';
				error = '';
				formOpen = false;
				formTemplate = null;
				deleteTarget = null;
				deleting = false;
				formHasPendingUpload = false;
				listHasPendingUpload = false;
				formEpoch++;
			}
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
				loading = true;
				void b.then((v) => applyTemplates(v, t.revision));
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		campaignRequest.abort();
		templateRequest.abort();
	});
	function current(epoch: number, key: string) {
		return !disposed && canReadTemplates && epoch === ownerEpoch && key === context;
	}
	function applyCampaign(v: Awaited<typeof data.campaign>, revision: number) {
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
	function applyTemplates(v: Awaited<typeof data.templates>, revision: number) {
		if (!templateRequest.isCurrent(revision)) return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== context) return;
		templates = v.data.record ?? [];
		loaded = true;
		error = '';
	}

	const hasPendingUpload = $derived(formHasPendingUpload || listHasPendingUpload);

	async function retryCampaign() {
		if (disposed || !canReadTemplates) return;
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
		if (disposed || !canReadTemplates) return;
		const ownerKey = context,
			t = templateRequest.begin();
		loading = true;
		error = '';
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

	function patchTemplate(updated: CertificateTemplateDetail) {
		if (disposed || !canReadTemplates || updated.campaignId !== campaignId) return;
		templateRequest.abort();
		loading = false;
		error = '';
		const current = templates.find((template) => template.id === updated.id);
		if (!current) {
			templates = [updated, ...templates];
			return;
		}
		const currentUpdatedAt = Date.parse(current.updatedAt);
		const incomingUpdatedAt = Date.parse(updated.updatedAt);
		if (
			Number.isFinite(currentUpdatedAt) &&
			Number.isFinite(incomingUpdatedAt) &&
			incomingUpdatedAt < currentUpdatedAt
		) {
			return;
		}
		templates = templates.map((template) => (template.id === updated.id ? updated : template));
	}

	function openCreate() {
		if (!canCreateTemplates || !loaded) return;
		formEpoch++;
		formHasPendingUpload = false;
		formTemplate = null;
		formOpen = true;
	}

	function openEdit(template: CertificateTemplateDetail) {
		if (!hasUpdatePermission || !template.capabilities.canUpdate) return;
		formEpoch++;
		formHasPendingUpload = false;
		formTemplate = template;
		formOpen = true;
	}

	function closeForm() {
		if (formHasPendingUpload) {
			formOpen = true;
			toast.error('แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนปิดแบบฟอร์ม');
			return;
		}
		formOpen = false;
		formTemplate = null;
	}

	function completeForm() {
		toast.success(formTemplate ? 'บันทึกข้อมูลแบบแล้ว' : 'สร้างแบบและแนบพื้นหลังแล้ว');
		closeForm();
	}

	async function handleDelete() {
		if (!deleteTarget || deleting || !deleteTarget.capabilities.canDelete) return;
		const epoch = ownerEpoch,
			key = context;
		const target = deleteTarget;
		deleting = true;
		try {
			const result = await deleteCertificateTemplate(target.id);
			if (!current(epoch, key)) return;
			templateRequest.abort();
			loading = false;
			if (result.disposition === 'deleted') {
				templates = templates.filter((template) => template.id !== target.id);
				toast.success('ลบแบบเกียรติบัตรแล้ว');
			} else {
				const updated = await getCertificateTemplate(target.id);
				if (!current(epoch, key)) return;
				patchTemplate(updated);
				toast.success('ปิดใช้แบบเกียรติบัตรแล้ว เพราะมีใบที่ออกด้วยแบบนี้');
			}
			deleteTarget = null;
		} catch (deleteError) {
			if (!current(epoch, key)) return;
			toast.error(deleteError instanceof Error ? deleteError.message : 'ลบแบบเกียรติบัตรไม่สำเร็จ');
		} finally {
			if (current(epoch, key)) deleting = false;
		}
	}

	beforeNavigate(({ cancel, shallow }) => {
		if (shallow) return;
		if (!hasPendingUpload) return;
		cancel();
		toast.error('แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนออกจากหน้านี้');
	});
</script>

<PageShell
	title="แบบเกียรติบัตร"
	description="แยกแบบตามบทบาทหรือรางวัล แล้วกำหนด PDF พื้นหลัง รูปประกอบ และฟอนต์ของแต่ละแบบ"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<FileBadge2 class="size-4" />
			{loaded ? `${templates.length} แบบ` : 'กำลังโหลดแบบ'}{campaign
				? ` · ${campaign.name}`
				: 'ในกิจกรรมนี้'}
		</div>
	{/snippet}

	{#snippet actions()}
		{#if canReadTemplates}<Button variant="outline" onclick={retryTemplates}>โหลดแบบใหม่</Button
			>{/if}
		{#if canCreateTemplates}
			<Button onclick={openCreate}>
				<Plus class="size-4" /> เพิ่มแบบเกียรติบัตร
			</Button>
		{/if}
	{/snippet}

	{#if !canReadTemplates}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูแบบเกียรติบัตร"
			description="สิทธิ์การอ่านและหน่วยงานเจ้าของกิจกรรมตรวจจาก backend"
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
		{#if loading && !loaded}<div role="status" aria-label="กำลังโหลดแบบเกียรติบัตร">
				<PageSkeleton variant="cards" />
			</div>{/if}
		{#if loading && loaded}<p role="status">กำลังอัปเดตแบบเกียรติบัตร</p>{/if}
		{#if error}
			<PageState
				variant="error"
				title="โหลดแบบเกียรติบัตรไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={retryTemplates}
			/>
		{/if}
		{#if loaded}
			{#key context}
				<CertificateTemplateList
					canManage={hasUpdatePermission}
					{campaignId}
					{templates}
					onpatched={patchTemplate}
					onedit={openEdit}
					ondelete={(template) => (deleteTarget = template)}
					oncreate={canCreateTemplates ? openCreate : undefined}
					onpendingchange={(pending) => (listHasPendingUpload = pending)}
				/>
			{/key}
		{/if}
	{/if}
</PageShell>

<Dialog.Root bind:open={formOpen} onOpenChange={(open) => !open && closeForm()}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>{formTemplate ? 'แก้ข้อมูลแบบเกียรติบัตร' : 'เพิ่มแบบเกียรติบัตร'}</Dialog.Title
			>
			<Dialog.Description>
				{formTemplate
					? 'ชื่อและประเภทผู้รับมีผลกับการเลือกแบบตอนเตรียมรายชื่อ'
					: 'ระบบจะสร้างโครงแบบก่อน แล้วอัปโหลดและตรวจ PDF พื้นหลังด้วยรหัสแบบเดียวกัน'}
			</Dialog.Description>
		</Dialog.Header>
		{#if formOpen && hasUpdatePermission && (formTemplate?.capabilities.canUpdate || canCreateTemplates)}
			{@const epoch = ownerEpoch}
			{@const key = context}
			{@const draft = formEpoch}
			{#key `${context}|${formEpoch}`}
				<CertificateTemplateForm
					{campaignId}
					template={formTemplate ?? undefined}
					onpatched={(updated) => current(epoch, key) && patchTemplate(updated)}
					onpendingchange={(pending) => (formHasPendingUpload = pending)}
					oncompleted={() => {
						if (current(epoch, key) && draft === formEpoch) completeForm();
					}}
					oncancel={closeForm}
				/>
			{/key}
		{/if}
	</Dialog.Content>
</Dialog.Root>

<AlertDialog.Root
	open={deleteTarget !== null}
	onOpenChange={(open) => !open && !deleting && (deleteTarget = null)}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ลบ “{deleteTarget?.name ?? ''}”?</AlertDialog.Title>
			<AlertDialog.Description>
				{#if (deleteTarget?.issuedCertificateCount ?? 0) > 0}
					แบบนี้มีเกียรติบัตรที่ออกแล้ว ระบบจะปิดใช้แทนการลบ เพื่อให้ใบเดิมยังเปิดและดาวน์โหลดได้
				{:else}
					PDF พื้นหลัง รูป และฟอนต์ที่แนบกับแบบร่างนี้จะถูกนำออกจากระบบด้วย
				{/if}
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={deleting}>ยกเลิก</AlertDialog.Cancel>
			<LoadingButton variant="destructive" loading={deleting} onclick={handleDelete}>
				<Trash2 class="size-4" />
				{(deleteTarget?.issuedCertificateCount ?? 0) > 0 ? 'ปิดใช้แบบ' : 'ลบแบบ'}
			</LoadingButton>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
