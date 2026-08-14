<script lang="ts">
	import { page } from '$app/state';
	import { PageShell } from '$lib/components/app-layout';
	import CertificateIssuedTable from '$lib/components/certificates/CertificateIssuedTable.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const campaignId = $derived(page.params.campaignId ?? '');
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canDownload = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_DOWNLOAD_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_DOWNLOAD_SCHOOL
		)
	);
	const canRevoke = $derived($can.has(PERMISSIONS.CERTIFICATE_REVOKE_SCHOOL));
</script>

<PageShell
	title="ใบที่ออกแล้ว"
	description="ค้นหา ดาวน์โหลด และตรวจสถานะเลขเกียรติบัตรของกิจกรรม โดยใบเก่าจะสร้างไฟล์จากแบบปัจจุบัน"
>
	<CertificateIssuedTable {campaignId} {canRead} {canDownload} {canRevoke} />
</PageShell>
