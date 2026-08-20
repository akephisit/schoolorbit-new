<script lang="ts">
	import type { LayoutProps } from './$types';
	import CertificateCampaignWorkspaceNav, {
		type CertificateCampaignSectionPath
	} from '$lib/components/certificates/CertificateCampaignWorkspaceNav.svelte';

	let { data, children }: LayoutProps = $props();

	const sectionPaths: CertificateCampaignSectionPath[] = [
		'/overview',
		'/templates',
		'/recipients',
		'/requests',
		'/issued'
	];
</script>

<div class="min-w-0">
	<CertificateCampaignWorkspaceNav campaignId={data.campaignId} {sectionPaths} />
	{@render children()}
</div>
