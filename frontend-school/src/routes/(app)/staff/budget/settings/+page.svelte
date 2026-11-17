<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import Settings from '#lib/components/budget/Settings.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${$can.has(PERMISSIONS.BUDGET_MANAGE_SCHOOL)}|${$can.has(PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL)}`}<Settings
		source={budgetSource(data.record, identity)}
		indexSource={budgetSource(data.index, identity)}
		selectedId={data.selectedId}
	/>{/key}
