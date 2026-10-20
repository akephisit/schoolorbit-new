<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import AdjustmentDetail from '#lib/components/budget/AdjustmentDetail.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${data.requestKey}`}<AdjustmentDetail
		id={data.id}
		source={budgetSource(data.record, identity)}
		processSource={budgetSource(data.process, identity)}
		fromSource={budgetSource(data.fromBudget, identity)}
		toSource={budgetSource(data.toBudget, identity)}
		catalogsSource={budgetSource(data.catalogs, identity)}
		returnTo={data.returnTo}
	/>{/key}
