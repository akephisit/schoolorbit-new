<script lang="ts" generics="T">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { careIdentity, readCare, type CareRead } from '#lib/features/student-care/access.js';
	let {
		initial,
		permissions,
		retry,
		children,
		variant = 'table',
		label = 'ข้อมูลดูแลนักเรียน'
	}: {
		initial: Promise<RouteLoadResult<CareRead<T>>>;
		permissions: string[];
		retry: (signal: AbortSignal) => Promise<T>;
		children: Snippet<[T]>;
		variant?: 'table' | 'form' | 'detail';
		label?: string;
	} = $props();
	let source = $state.raw(untrack(() => initial));
	let previous = untrack(() => initial);
	const request = new LatestRequest();
	$effect.pre(() => {
		if (initial !== previous) {
			previous = initial;
			request.abort();
			source = initial;
		}
	});
	onDestroy(() => request.abort());
	function reload() {
		const token = request.begin();
		source = captureRouteLoad(
			readCare(permissions, () => retry(token.signal)),
			`โหลด${label}ไม่สำเร็จ`
		);
	}
</script>

{#await source}
	<PageSkeleton {variant} />
{:then result}
	{#if result.ok && result.data.identity !== $careIdentity}
		<PageSkeleton {variant} />
	{:else if result.ok && result.data.resource !== null}
		{@render children(result.data.resource)}
	{:else if result.ok}
		<PageState variant="permission" title="ไม่มีสิทธิ์ใช้งานส่วนนี้" />
	{:else}
		<PageState
			variant="error"
			title={`โหลด${label}ไม่สำเร็จ`}
			description={result.error}
			actionLabel={`ลองใหม่: ${label}`}
			onaction={reload}
		/>
	{/if}
{/await}
