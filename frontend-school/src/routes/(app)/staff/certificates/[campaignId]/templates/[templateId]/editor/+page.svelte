<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		createCertificateTemplatePreviewManifest,
		getCertificateTemplate,
		getCertificateTemplateVariableCatalog,
		type CertificateRenderManifest,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateEditor from '$lib/components/certificates/editor/CertificateEditor.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let template = $state.raw<CertificateTemplateDetail | null>(null);
	let manifest = $state.raw<CertificateRenderManifest | null>(null);
	let variables = $state.raw<string[]>([]);
	let loading = $state(true);
	let error = $state('');
	let loadGeneration = 0;

	const campaignId = $derived(page.params.campaignId ?? '');
	const templateId = $derived(page.params.templateId ?? '');
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);

	async function loadEditor(targetCampaignId: string, targetTemplateId: string) {
		const generation = ++loadGeneration;
		loading = true;
		error = '';
		template = null;
		manifest = null;
		variables = [];
		if (!canRead || !targetCampaignId || !targetTemplateId) {
			if (generation === loadGeneration) loading = false;
			return;
		}
		try {
			const [loadedTemplate, catalog, loadedManifest] = await Promise.all([
				getCertificateTemplate(targetTemplateId),
				getCertificateTemplateVariableCatalog(targetTemplateId),
				createCertificateTemplatePreviewManifest(targetTemplateId, { previewKind: 'short' })
			]);
			if (
				generation !== loadGeneration ||
				targetCampaignId !== campaignId ||
				targetTemplateId !== templateId
			) {
				return;
			}
			if (loadedTemplate.campaignId !== targetCampaignId) {
				throw new Error('แบบเกียรติบัตรนี้ไม่ได้อยู่ในกิจกรรมตาม URL');
			}
			template = loadedTemplate;
			manifest = loadedManifest;
			variables = catalog.variables;
		} catch (loadError) {
			if (
				generation !== loadGeneration ||
				targetCampaignId !== campaignId ||
				targetTemplateId !== templateId
			) {
				return;
			}
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถเปิด editor ได้';
		} finally {
			if (
				generation === loadGeneration &&
				targetCampaignId === campaignId &&
				targetTemplateId === templateId
			) {
				loading = false;
			}
		}
	}

	afterNavigate(() => {
		void loadEditor(campaignId, templateId);
	});
</script>

<PageShell
	title={template ? `ออกแบบ · ${template.name}` : 'ออกแบบเกียรติบัตร'}
	description="วางข้อความ ตัวแปร รูปภาพ และ QR Code บน PDF พื้นหลัง แล้วตรวจด้วย renderer เดียวกับไฟล์จริง"
	backHref={resolve(`/staff/certificates/${campaignId}/templates` as '/staff/certificates')}
	backLabel="กลับไปแบบเกียรติบัตร"
	contentClass="space-y-0 pb-3"
>
	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์เปิด editor"
			description="สิทธิ์อ่านและหน่วยงานเจ้าของกิจกรรมตรวจจาก backend"
		/>
	{:else if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="เปิด editor ไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadEditor(campaignId, templateId)}
		/>
	{:else if template && manifest}
		{#if template.capabilities.canUpdate}
			{#key template.id}
				<CertificateEditor {template} initialManifest={manifest} {variables} />
			{/key}
		{:else}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์แก้แบบนี้"
				description="คุณยังดูข้อมูลกิจกรรมได้ แต่ backend ไม่อนุญาตให้แก้ layout ของหน่วยงานนี้"
			/>
		{/if}
	{/if}
</PageShell>
