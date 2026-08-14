<script lang="ts">
	import { afterNavigate, beforeNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import {
		deleteCertificateTemplate,
		getCertificateCampaign,
		getCertificateTemplate,
		listCertificateTemplates,
		type CertificateCampaignDetail,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateTemplateForm from '$lib/components/certificates/CertificateTemplateForm.svelte';
	import CertificateTemplateList from '$lib/components/certificates/CertificateTemplateList.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { FileBadge2, Plus, Trash2 } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let campaign = $state.raw<CertificateCampaignDetail | null>(null);

	const campaignId = $derived(page.params.campaignId ?? '');
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
	let loadGeneration = 0;

	const hasPendingUpload = $derived(formHasPendingUpload || listHasPendingUpload);

	async function loadWorkspace(targetCampaignId: string) {
		const generation = ++loadGeneration;
		if (!canReadTemplates) {
			if (generation === loadGeneration) loading = false;
			return;
		}
		loading = true;
		error = '';
		campaign = null;
		templates = [];
		formOpen = false;
		formTemplate = null;
		deleteTarget = null;
		try {
			const [loadedCampaign, loadedTemplates] = await Promise.all([
				getCertificateCampaign(targetCampaignId),
				listCertificateTemplates(targetCampaignId)
			]);
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			campaign = loadedCampaign;
			templates = loadedTemplates;
		} catch (loadError) {
			if (generation !== loadGeneration || targetCampaignId !== campaignId) return;
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดแบบเกียรติบัตรได้';
		} finally {
			if (generation === loadGeneration && targetCampaignId === campaignId) loading = false;
		}
	}

	function patchTemplate(updated: CertificateTemplateDetail) {
		if (updated.campaignId !== campaignId) return;
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
		if (formTemplate?.id === updated.id) formTemplate = updated;
	}

	function openCreate() {
		if (!canCreateTemplates) return;
		formHasPendingUpload = false;
		formTemplate = null;
		formOpen = true;
	}

	function openEdit(template: CertificateTemplateDetail) {
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
		if (!deleteTarget || deleting) return;
		const target = deleteTarget;
		deleting = true;
		try {
			const result = await deleteCertificateTemplate(target.id);
			if (result.disposition === 'deleted') {
				templates = templates.filter((template) => template.id !== target.id);
				toast.success('ลบแบบเกียรติบัตรแล้ว');
			} else {
				const updated = await getCertificateTemplate(target.id);
				patchTemplate(updated);
				toast.success('ปิดใช้แบบเกียรติบัตรแล้ว เพราะมีใบที่ออกด้วยแบบนี้');
			}
			deleteTarget = null;
		} catch (deleteError) {
			toast.error(deleteError instanceof Error ? deleteError.message : 'ลบแบบเกียรติบัตรไม่สำเร็จ');
		} finally {
			deleting = false;
		}
	}

	beforeNavigate(({ cancel }) => {
		if (!hasPendingUpload) return;
		cancel();
		toast.error('แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนออกจากหน้านี้');
	});

	afterNavigate(() => {
		void loadWorkspace(campaignId);
	});
</script>

<PageShell
	title="แบบเกียรติบัตร"
	description="แยกแบบตามบทบาทหรือรางวัล แล้วกำหนด PDF พื้นหลัง รูปประกอบ และฟอนต์ของแต่ละแบบ"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<FileBadge2 class="size-4" />
			{templates.length} แบบ{campaign ? ` · ${campaign.name}` : 'ในกิจกรรมนี้'}
		</div>
	{/snippet}

	{#snippet actions()}
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
	{:else if loading}
		<PageSkeleton variant="cards" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดแบบเกียรติบัตรไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(campaignId)}
		/>
	{:else}
		<CertificateTemplateList
			{campaignId}
			{templates}
			onpatched={patchTemplate}
			onedit={openEdit}
			ondelete={(template) => (deleteTarget = template)}
			oncreate={canCreateTemplates ? openCreate : undefined}
			onpendingchange={(pending) => (listHasPendingUpload = pending)}
		/>
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
		{#if formOpen}
			{#key formTemplate?.id ?? 'new-template'}
				<CertificateTemplateForm
					{campaignId}
					template={formTemplate ?? undefined}
					onpatched={patchTemplate}
					onpendingchange={(pending) => (formHasPendingUpload = pending)}
					oncompleted={completeForm}
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
