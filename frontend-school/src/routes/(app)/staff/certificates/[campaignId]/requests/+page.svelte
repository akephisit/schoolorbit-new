<script lang="ts">
	import { page } from '$app/state';
	import CertificateCampaignRequests from '$lib/components/certificates/CertificateCampaignRequests.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const campaignId = $derived(page.params.campaignId ?? '');
	const canSubmit = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_SUBMIT_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_SUBMIT_SCHOOL
		)
	);
</script>

<CertificateCampaignRequests {campaignId} {canSubmit} />
