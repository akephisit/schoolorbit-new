<script lang="ts">
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { providePersonalSupervisionReturn } from '#lib/supervision/personal-return.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	const personalReturn = providePersonalSupervisionReturn();

	$effect.pre(() => {
		void $authStore;
		void $userPermissions;
		const owner = appIdentityKey();
		return () => {
			if (owner !== appIdentityKey()) personalReturn.clear();
		};
	});
</script>

{@render children()}
