<script lang="ts">
	import type { PageProps } from './$types';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { budgetSource } from '#lib/budget/route-data.js';
	import RequestDetail from '#lib/components/budget/RequestDetail.svelte';
	let { data }: PageProps = $props();
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key `${identity}|${data.id}`}<RequestDetail
		id={data.id}
		source={budgetSource(data.record, identity)}
		processSource={budgetSource(data.process, identity)}
		paymentsSource={budgetSource(data.payments, identity)}
		returnTo={data.returnTo}
	/>{/key}
