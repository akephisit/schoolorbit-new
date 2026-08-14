<script lang="ts">
	import { page } from '$app/state';
	import CertificateIssueRequestReview from '$lib/components/certificates/CertificateIssueRequestReview.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const requestId = $derived(page.params.requestId ?? '');
	const canIssue = $derived($can.has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL));
</script>

<CertificateIssueRequestReview {requestId} {canIssue} />
