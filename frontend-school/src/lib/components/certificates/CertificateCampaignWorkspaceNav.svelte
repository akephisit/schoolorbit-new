<script lang="ts" module>
	export type CertificateCampaignSectionPath =
		| '/overview'
		| '/templates'
		| '/recipients'
		| '/requests'
		| '/issued';
</script>

<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { cn } from '$lib/utils';
	import { Award, FileBadge2, LayoutDashboard, Send, UsersRound } from 'lucide-svelte';

	let {
		campaignId,
		sectionPaths = ['/overview', '/templates', '/recipients', '/requests', '/issued']
	}: {
		campaignId: string;
		sectionPaths?: CertificateCampaignSectionPath[];
	} = $props();

	const sectionMeta = {
		'/overview': { label: 'ภาพรวม', icon: LayoutDashboard },
		'/templates': { label: 'แบบเกียรติบัตร', icon: FileBadge2 },
		'/recipients': { label: 'รายชื่อผู้รับ', icon: UsersRound },
		'/requests': { label: 'คำขอออก', icon: Send },
		'/issued': { label: 'ใบที่ออกแล้ว', icon: Award }
	} satisfies Record<CertificateCampaignSectionPath, { label: string; icon: typeof Award }>;

	const items = $derived(
		sectionPaths.map((sectionPath) => ({
			...sectionMeta[sectionPath],
			sectionPath
		}))
	);
</script>

<nav class="overflow-x-auto border-b bg-background" aria-label="พื้นที่จัดการชุดออกเกียรติบัตร">
	<div class="flex min-w-max gap-1 px-4 lg:px-6">
		{#each items as item (item.sectionPath)}
			{@const ItemIcon = item.icon}
			<a
				href={resolve(
					`/staff/certificates/${campaignId}${item.sectionPath}` as '/staff/certificates'
				)}
				aria-current={page.url.pathname === `/staff/certificates/${campaignId}${item.sectionPath}`
					? 'page'
					: undefined}
				class={cn(
					'relative inline-flex h-12 items-center gap-2 px-3 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground',
					page.url.pathname === `/staff/certificates/${campaignId}${item.sectionPath}` &&
						'text-foreground after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:rounded-full after:bg-primary'
				)}
			>
				<ItemIcon class="size-4" />
				{item.label}
			</a>
		{/each}
	</div>
</nav>
