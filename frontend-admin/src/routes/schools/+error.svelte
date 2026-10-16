<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
</script>

<svelte:head
	><title>รายชื่อโรงเรียนไม่พร้อมใช้งาน — SchoolOrbit</title><meta
		name="robots"
		content="noindex"
	/></svelte:head
>
<main class="mx-auto max-w-2xl px-6 py-24 text-[#163441]">
	<h1 class="text-2xl font-semibold">ยังแสดงรายชื่อโรงเรียนไม่ได้</h1>
	<p class="mt-4 text-[#54717e]">{page.error?.message}</p>
	<a
		href={resolve('schools')}
		data-sveltekit-reload
		class="mt-8 inline-block text-[#237c9b] underline">ลองใหม่อีกครั้ง</a
	>
	<a href={resolve('/')} class="ml-6 text-[#237c9b] underline">กลับหน้าหลัก</a>
</main>
