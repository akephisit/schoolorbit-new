<script lang="ts">
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import CreateForm from '#lib/components/correspondence/CreateForm.svelte';
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
</script>

{#key identity}<CreateForm />{/key}
