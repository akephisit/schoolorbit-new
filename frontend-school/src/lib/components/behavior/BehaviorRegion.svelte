<script lang="ts" generics="T">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		behaviorIdentity,
		readBehavior,
		type BehaviorRead
	} from '#lib/features/behavior/access.js';
	let {
		initial,
		permissions,
		retry,
		children,
		variant = 'table'
	}: {
		initial: Promise<RouteLoadResult<BehaviorRead<T>>>;
		permissions: string[];
		retry: (signal: AbortSignal) => Promise<T>;
		children: Snippet<[T]>;
		variant?: 'table' | 'form' | 'detail';
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
			readBehavior(permissions, () => retry(token.signal)),
			'โหลดข้อมูลไม่สำเร็จ'
		);
	}
</script>

{#await source}<PageSkeleton
		{variant}
	/>{:then result}{#if result.ok && result.data.identity !== $behaviorIdentity}<PageSkeleton
			{variant}
		/>{:else if result.ok && result.data.resource}{@render children(
			result.data.resource
		)}{:else if result.ok}<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ใช้งานส่วนนี้"
		/>{:else}<PageState
			variant="error"
			title="โหลดข้อมูลไม่สำเร็จ"
			description={result.error}
			actionLabel="ลองโหลดใหม่"
			onaction={reload}
		/>{/if}{/await}
