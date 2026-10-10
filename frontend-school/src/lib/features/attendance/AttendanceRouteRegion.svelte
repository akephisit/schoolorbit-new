<script lang="ts" generics="T">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import {
		attendanceIdentity,
		captureAttendanceRead,
		type AttendanceRead
	} from './attendance-access.js';
	let {
		initial,
		retry,
		children,
		variant = 'table',
		errorTitle = 'โหลดข้อมูลเช็คชื่อไม่ได้',
		retryLabel = 'ลองโหลดข้อมูลใหม่'
	}: {
		initial: Promise<RouteLoadResult<AttendanceRead<T>>>;
		retry: (signal: AbortSignal) => Promise<T>;
		children: Snippet<[T]>;
		variant?: 'table' | 'form' | 'detail';
		errorTitle?: string;
		retryLabel?: string;
	} = $props();
	let source = $state.raw(untrack(() => initial));
	let previous = untrack(() => initial);
	let controller = new AbortController();
	$effect.pre(() => {
		if (initial !== previous) {
			previous = initial;
			controller.abort();
			controller = new AbortController();
			source = initial;
		}
	});
	onDestroy(() => controller.abort());
	function reload() {
		const identityKey = $attendanceIdentity;
		source = captureRouteLoad(
			captureAttendanceRead(identityKey, retry(controller.signal), errorTitle),
			errorTitle
		);
	}
</script>

{#await source}<PageSkeleton {variant} />{:then result}
	{#if result.ok && result.data.identityKey !== $attendanceIdentity}<PageSkeleton {variant} />
	{:else if result.ok && result.data.error}<PageState
			variant="error"
			title={errorTitle}
			description={result.data.error}
			actionLabel={retryLabel}
			onaction={reload}
		/>
	{:else if result.ok && result.data.resource}{@render children(result.data.resource)}
	{:else if result.ok}<PageState variant="permission" title="ไม่มีสิทธิ์ใช้งานส่วนนี้" />
	{:else}<PageState
			variant="error"
			title={errorTitle}
			description={result.error}
			actionLabel={retryLabel}
			onaction={reload}
		/>{/if}
{/await}
