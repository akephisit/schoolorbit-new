<script lang="ts" generics="T">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	let {
		initial,
		retry,
		children,
		variant = 'table'
	}: {
		initial: Promise<RouteLoadResult<T | null>>;
		retry: (signal: AbortSignal) => Promise<T>;
		children: Snippet<[T]>;
		variant?: 'table' | 'form' | 'detail';
	} = $props();
	let source = $state.raw(untrack(() => initial));
	const controller = new AbortController();
	onDestroy(() => controller.abort());
	function reload() {
		source = captureRouteLoad(retry(controller.signal), 'โหลดข้อมูลเช็คชื่อไม่ได้');
	}
</script>

{#await source}<PageSkeleton {variant} />{:then result}
	{#if result.ok && result.data}{@render children(result.data)}
	{:else if result.ok}<PageState variant="permission" title="ไม่มีสิทธิ์ใช้งานส่วนนี้" />
	{:else}<PageState
			variant="error"
			title="โหลดข้อมูลเช็คชื่อไม่ได้"
			description={result.error}
			actionLabel="ลองโหลดข้อมูลใหม่"
			onaction={reload}
		/>{/if}
{/await}
