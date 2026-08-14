<script lang="ts">
	import { page } from '$app/state';
	import CertificateRecipientWorkspace from '$lib/components/certificates/CertificateRecipientWorkspace.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const campaignId = $derived(page.params.campaignId ?? '');
	const canReadCandidates = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const hasUpdatePermission = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
		)
	);
</script>

<CertificateRecipientWorkspace {campaignId} {canReadCandidates} {hasUpdatePermission} />
