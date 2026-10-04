<script lang="ts">
	import type { PublicSchoolInfo } from '#lib/api/school.js';
	import PublicSchoolLogo from './PublicSchoolLogo.svelte';
	let { info }: { info: PublicSchoolInfo } = $props();
</script>

<svelte:head
	><title>{info.schoolName || 'เว็บไซต์โรงเรียน'} — ข้อมูลและบริการสาธารณะ</title></svelte:head
>
<div
	class="school-identity public-enter grid grid-cols-[auto_minmax(0,1fr)] items-center gap-4 sm:gap-6"
>
	<div class="relative w-14 self-stretch sm:w-20 lg:w-24" data-testid="hero-school-crest">
		<PublicSchoolLogo
			fileId={info.logoFileId}
			class="absolute inset-0 size-full object-contain drop-shadow-sm"
		/>
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
