<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore.user;
		void $can;
		return appIdentityKey();
	});

	import { page } from '$app/state';
	import CertificateCampaignRequests from '#lib/components/certificates/CertificateCampaignRequests.svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	const campaignId = $derived(page.params.campaignId ?? '');
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canSubmit = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_SUBMIT_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_SUBMIT_SCHOOL
		)
	);
</script>

<CertificateCampaignRequests
	{canRead}
	{identityKey}
	initialRequests={data.requests}
	{campaignId}
	{canSubmit}
/>
