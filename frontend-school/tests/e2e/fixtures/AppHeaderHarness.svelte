<script lang="ts">
	import Header from '#lib/components/layout/Header.svelte';
	import { applyPersonalMode } from '#lib/appearance/mode.js';
	import { uiPreferences } from '#lib/stores/ui-preferences.js';
	import {
		createAcademicContextStore,
		setAcademicContextStore
	} from '#lib/academic-context/store.js';
	import { authStore } from '#lib/stores/auth.js';
	setAcademicContextStore(createAcademicContextStore({ navigate: async () => {} }));
	authStore.clearUser();
	let menuOpen = $state(false);
	$effect.pre(() => {
		applyPersonalMode($uiPreferences.theme, '/staff');
	});
</script>

<Header onMenuClick={() => (menuOpen = !menuOpen)} />
<main class="p-4" data-testid="header-fixture">{menuOpen ? 'เมนูเปิดแล้ว' : 'เมนูปิดอยู่'}</main>
