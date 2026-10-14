<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import CatalogWorkspace from '#lib/components/supply-catalog/CatalogWorkspace.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		if (!$authStore.user || !$can.has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL)) return '';
		return appIdentityKey();
	});
</script>

{#if $can.has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL)}
	{#key identity}<CatalogWorkspace
			{identity}
			initialItems={data.items}
			initialCategories={data.categories}
			query={data.query}
			requestKey={data.requestKey}
			canManage={$can.has(PERMISSIONS.SUPPLY_CATALOG_MANAGE_SCHOOL)}
			canPropose={$can.has(PERMISSIONS.SUPPLY_CATALOG_REQUEST_OWN)}
			canApprove={$can.has(PERMISSIONS.SUPPLY_CATALOG_APPROVE_SCHOOL)}
			userId={$authStore.user?.id ?? ''}
		/>{/key}
{:else}<PageShell title="คลังพัสดุ"
		><PageState variant="permission" title="ไม่มีสิทธิ์ดูคลังพัสดุ" /></PageShell
	>{/if}
