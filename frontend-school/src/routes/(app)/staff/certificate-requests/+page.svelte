<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	let { data }: PageProps = $props();
	const identityKey = $derived.by(() => {
		void $authStore.user;
		void $can;
		return appIdentityKey();
	});

	import CertificateIssueQueue from '$lib/components/certificates/CertificateIssueQueue.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const canIssue = $derived($can.has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL));
</script>

<CertificateIssueQueue
	{identityKey}
	initialRequests={data.requests}
	selectedStatus={data.status}
	{canIssue}
/>
