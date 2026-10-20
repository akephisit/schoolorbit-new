<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import AdjustmentList from '#lib/components/budget/AdjustmentList.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${data.requestKey}`}<AdjustmentList
		source={budgetSource(data.record, identity)}
		query={data.query}
	/>{/key}
