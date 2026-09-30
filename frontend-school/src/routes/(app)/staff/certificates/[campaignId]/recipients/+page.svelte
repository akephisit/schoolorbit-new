<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { page } from '$app/state';
	import CertificateRecipientWorkspace from '$lib/components/certificates/CertificateRecipientWorkspace.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const campaignId = $derived(page.params.campaignId ?? '');
	const canReadCandidates = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canUpdate = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
		)
	);
	const canSubmit = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_SUBMIT_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_SUBMIT_SCHOOL
		)
	);
</script>

<CertificateRecipientWorkspace
	{campaignId}
	{canReadCandidates}
	{canUpdate}
	{canSubmit}
	{identityKey}
	initialCampaign={data.campaign}
	initialTemplates={data.templates}
	initialCandidates={data.candidates}
/>
