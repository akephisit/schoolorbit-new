<script lang="ts">
	import { onMount } from 'svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import AppearanceEditor from '#lib/components/appearance/AppearanceEditor.svelte';
	import { APPEARANCE_STORAGE_KEY, readStoredAppearance } from '#lib/design-preview/appearance.js';
	import { DEFAULT_APPEARANCE, type SchoolAppearance } from '#lib/appearance/model.js';
	let draft = $state<SchoolAppearance>({ ...DEFAULT_APPEARANCE });
	let ready = $state(false),
		message = $state('');
	onMount(() => {
		try {
			const saved = readStoredAppearance(localStorage.getItem(APPEARANCE_STORAGE_KEY));
			if (saved) {
				const { palette, font, density, primary } = saved;
				draft = { palette, font, density, primary };
			}
		} catch {
			message = 'อ่านตัวอย่างที่บันทึกไว้ไม่ได้ คุณยังทดลองปรับรูปแบบได้';
		}
		ready = true;
	});
	function saveExample() {
		try {
			localStorage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify({ version: 1, ...draft }));
			message = 'บันทึกตัวอย่างในเบราว์เซอร์นี้แล้ว';
		} catch {
			message = 'บันทึกตัวอย่างไม่สำเร็จ แบบร่างของคุณยังอยู่';
		}
	}
</script>

<PageShell
	title="รูปแบบและตัวอักษร"
	description="พื้นที่ทดลองดีไซน์ · บันทึกเฉพาะเบราว์เซอร์นี้ ยังไม่เปลี่ยนการตั้งค่าจริงของโรงเรียน"
>
	<p role="status">{message}</p>
	<AppearanceEditor bind:draft disabled={!ready} onsave={saveExample} saveLabel="บันทึกตัวอย่าง" />
</PageShell>
