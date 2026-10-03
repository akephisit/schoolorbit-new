<script lang="ts">
	import type { PublicSchoolInfo } from '$lib/api/school';
	import { publicFileUrl } from '$lib/api/files';
	import { School } from '@lucide/svelte';
	let { info }: { info: PublicSchoolInfo } = $props();
	let failedLogo = $state<string | null>(null);
</script>

<svelte:head
	><title>{info.schoolName || 'เว็บไซต์โรงเรียน'} — ข้อมูลและบริการสาธารณะ</title></svelte:head
>
<div class="flex flex-col items-start gap-5 sm:flex-row sm:items-center sm:gap-7">
	<div
		class="flex size-20 shrink-0 items-center justify-center rounded-2xl border border-border bg-card p-3 sm:size-24"
	>
		{#if info.logoFileId && failedLogo !== info.logoFileId}<img
				src={publicFileUrl(info.logoFileId)}
				alt="โลโก้โรงเรียน"
				width="72"
				height="72"
				class="size-full object-contain"
				onerror={() => (failedLogo = info.logoFileId ?? null)}
			/>{:else}<School class="size-10 text-primary" strokeWidth={1.5} />{/if}
	</div>
	<div class="min-w-0">
		<h1
			class="break-words text-3xl font-semibold leading-snug tracking-tight sm:text-4xl lg:text-5xl"
		>
			{info.schoolName || 'เว็บไซต์โรงเรียน'}
		</h1>
		<p class="mt-3 text-base leading-relaxed text-muted-foreground sm:text-lg">
			รู้จักโรงเรียนของเรา ผ่านข้อมูลที่เปิดให้ทุกคนเข้าถึง
		</p>
	</div>
</div>
