<script lang="ts" generics="T">
	import { untrack, type Snippet } from 'svelte';
	import { PageState } from '$lib/components/app-state';
	import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';

	let {
		source,
		retry,
		label,
		children,
		skeleton
	}: {
		source: Promise<RouteLoadResult<T>>;
		retry: (signal: AbortSignal) => Promise<T>;
		label: string;
		children: Snippet<[T]>;
		skeleton: Snippet;
	} = $props();
	let replacement = $state<Promise<RouteLoadResult<T>> | null>(null);
	let controller: AbortController | undefined;
	const operation = $derived(replacement ?? source);

	$effect.pre(() => {
		const _source = source;
		untrack(() => {
			replacement = null;
		});
		return () => controller?.abort();
	});

	function retryRegion() {
		controller?.abort();
		controller = new AbortController();
		replacement = captureRouteLoad(retry(controller.signal), `โหลด${label}ไม่สำเร็จ`);
	}
</script>

{#await operation}
	<div role="status" aria-label={`กำลังโหลด${label}`} aria-busy="true">
		{@render skeleton()}
	</div>
{:then result}
	{#if result.ok}
		{@render children(result.data)}
	{:else}
		<PageState
			variant="error"
			title={`โหลด${label}ไม่สำเร็จ`}
			description="ลองโหลดส่วนนี้ใหม่อีกครั้ง บริการส่วนอื่นยังใช้งานได้"
			actionLabel={`ลองใหม่: ${label}`}
			onaction={retryRegion}
		/>
	{/if}
{/await}
