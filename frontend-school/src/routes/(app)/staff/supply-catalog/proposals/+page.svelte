<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import ProposalWorkspace from '#lib/components/supply-catalog/ProposalWorkspace.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		if (!$authStore.user || !$can.has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL)) return '';
		return appIdentityKey();
	});
	const canPropose = $derived($can.has(PERMISSIONS.SUPPLY_CATALOG_REQUEST_OWN));
	const canApprove = $derived($can.has(PERMISSIONS.SUPPLY_CATALOG_APPROVE_SCHOOL));
	const allowed = $derived(
		$can.has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL) &&
			(data.query.review ? canApprove : canPropose)
	);
</script>

<PageShell
	title={data.title}
	description="ติดตามข้อเสนอและรับข้อมูลที่ตรวจแล้วเข้าคลังพัสดุ"
	backHref="/staff/supply-catalog"
>
	{#if allowed}{#key identity}<ProposalWorkspace
				{identity}
				{canPropose}
				{canApprove}
				initialProposals={data.proposals}
				initialCategories={data.categories}
				query={data.query}
				requestKey={data.requestKey}
			/>{/key}{:else}<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ใช้งานข้อเสนอพัสดุ"
		/>{#if canApprove}<Button
				variant="outline"
				href={`${'/staff/supply-catalog/proposals'}?review=true`}>ตรวจข้อเสนอรออนุมัติ</Button
			>{/if}{/if}
</PageShell>
