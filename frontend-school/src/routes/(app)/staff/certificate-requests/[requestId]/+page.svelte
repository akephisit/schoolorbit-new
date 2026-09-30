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

	import { page } from '$app/state';
	import CertificateIssueRequestReview from '$lib/components/certificates/CertificateIssueRequestReview.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const requestId = $derived(page.params.requestId ?? '');
	const canIssue = $derived($can.has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL));
</script>

<CertificateIssueRequestReview {identityKey} initialRequest={data.request} {requestId} {canIssue} />
