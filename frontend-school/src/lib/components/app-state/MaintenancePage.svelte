<script lang="ts">
	import { RefreshCw } from 'lucide-svelte';

	let { releaseId }: { releaseId: string | null } = $props();
</script>

<svelte:head>
	<title>กำลังปรับปรุงระบบ - SchoolOrbit</title>
</svelte:head>

<main
	class="fixed inset-0 z-[100] flex min-h-dvh items-center justify-center bg-slate-50 px-6 py-12 text-slate-900"
	aria-live="polite"
	aria-busy="true"
>
	<section
		class="w-full max-w-md rounded-3xl border border-slate-200 bg-white p-8 text-center shadow-xl"
	>
		<div
			class="mx-auto mb-6 flex size-16 items-center justify-center rounded-2xl bg-blue-50 text-blue-600"
		>
			<RefreshCw class="size-8 animate-spin" aria-hidden="true" />
		</div>
		<h1 class="text-2xl font-semibold tracking-tight">ระบบกำลังปรับปรุง</h1>
		<p class="mt-3 text-sm leading-6 text-slate-600">
			กำลังติดตั้งเวอร์ชันใหม่ กรุณารอสักครู่ หน้านี้จะโหลดใหม่โดยอัตโนมัติเมื่อระบบพร้อมใช้งาน
		</p>
		{#if releaseId}
			<p class="mt-5 font-mono text-xs text-slate-400">รุ่น {releaseId.slice(0, 12)}</p>
		{/if}
	</section>
</main>
