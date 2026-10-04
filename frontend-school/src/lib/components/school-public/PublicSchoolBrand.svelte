<script lang="ts">
	import type { PublicSchoolInfo } from '#lib/api/school.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import PublicSchoolLogo from './PublicSchoolLogo.svelte';
	let { operation }: { operation: Promise<RouteLoadResult<PublicSchoolInfo>> } = $props();
</script>

{#await operation}
	<Skeleton class="size-9 shrink-0" /><Skeleton class="h-4 w-28 max-w-full" />
{:then result}
	<PublicSchoolLogo
		fileId={result.ok ? result.data.logoFileId : null}
		class="size-9 shrink-0 object-contain sm:size-10"
	/>
	<span class="min-w-0 truncate text-sm sm:text-base"
		>{result.ok ? result.data.schoolName || 'เว็บไซต์โรงเรียน' : 'เว็บไซต์โรงเรียน'}</span
	>
{/await}
