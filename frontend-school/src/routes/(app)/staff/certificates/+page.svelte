<script lang="ts">
	import { onMount } from 'svelte';
	import { listCertificateCampaigns, type CertificateCampaignSummary } from '$lib/api/certificates';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import CertificateCampaignList from '$lib/components/certificates/CertificateCampaignList.svelte';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Plus } from 'lucide-svelte';

	const canReadCampaigns = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canCreateCampaign = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_CREATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_CREATE_SCHOOL
		)
	);

	let campaigns: CertificateCampaignSummary[] = $state.raw([]);
	let loading = $state(true);
	let error = $state('');

	async function loadCampaigns() {
		if (!canReadCampaigns) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			campaigns = await listCertificateCampaigns();
		} catch (loadError) {
			error = loadError instanceof Error ? loadError.message : 'ไม่สามารถโหลดชุดออกเกียรติบัตรได้';
		} finally {
			loading = false;
		}
	}

	onMount(loadCampaigns);
</script>

<PageShell
	title="ชุดออกเกียรติบัตร"
	description="จัดการกิจกรรม แม่แบบ รายชื่อ และคำขอออกเกียรติบัตรตามขอบเขตหน่วยงาน"
>
	{#snippet actions()}
		{#if canCreateCampaign}
			<Button href="/staff/certificates/new">
				<Plus class="size-4" />
				สร้างกิจกรรม
			</Button>
		{/if}
	{/snippet}

	{#if !canReadCampaigns}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูชุดออกเกียรติบัตร"
			description="ต้องมีสิทธิ์อ่านระดับหน่วยงานหรือระดับโรงเรียน จึงจะเปิดพื้นที่จัดการนี้ได้"
		/>
	{:else if loading}
		<PageSkeleton variant="cards" rows={4} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดชุดออกเกียรติบัตรไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadCampaigns}
		/>
	{:else}
		<CertificateCampaignList {campaigns} canCreate={canCreateCampaign} />
	{/if}
</PageShell>
