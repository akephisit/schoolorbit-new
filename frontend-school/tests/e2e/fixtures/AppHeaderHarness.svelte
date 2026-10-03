<script lang="ts">
	import Header from '$lib/components/layout/Header.svelte';
	import { createAcademicContextStore, setAcademicContextStore } from '$lib/academic-context/store';
	import { authStore } from '$lib/stores/auth';
	setAcademicContextStore(createAcademicContextStore({ navigate: async () => {} }));
	authStore.clearUser();
	let menuOpen = $state(false);
</script>

<Header onMenuClick={() => (menuOpen = !menuOpen)} />
<main class="p-4" data-testid="header-fixture">{menuOpen ? 'เมนูเปิดแล้ว' : 'เมนูปิดอยู่'}</main>
