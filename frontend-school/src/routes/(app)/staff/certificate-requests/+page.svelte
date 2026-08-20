<script lang="ts">
	import CertificateIssueQueue from '$lib/components/certificates/CertificateIssueQueue.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const canIssue = $derived($can.has(PERMISSIONS.CERTIFICATE_ISSUE_SCHOOL));
</script>

<CertificateIssueQueue {canIssue} />
