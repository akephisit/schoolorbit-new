<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { isValid, parseISO } from 'date-fns';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import SupervisionCalendarSchedule from '#lib/components/supervision/SupervisionCalendarSchedule.svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	type ScheduleSource = NonNullable<PageProps['data']['schedule']>;
	type ScheduleRead = NonNullable<Awaited<ScheduleSource>>;
	let completed = $state.raw<{ source: ScheduleSource; read: ScheduleRead }>();
	let selection = $state('');

	const identity = $derived.by(() => {
		void $userPermissions;
		return $authStore.isAuthenticated && $authStore.user?.user_type === 'staff'
			? appIdentityKey()
			: '';
	});
	const result = $derived(
		completed && completed.source === data.schedule && completed.read.identity === identity
			? completed.read.result
			: null
	);
	const entries = $derived(result?.ok ? result.data : []);
	const cycles = $derived(
		Array.from(
			new Map(entries.map(({ cycleId, cycleTitle }) => [cycleId, cycleTitle])),
			([id, title]) => ({ id, title })
		)
	);
	const selectedDate = $derived.by(() => {
		const date = page.url.searchParams.get('date');
		if (selection.startsWith(`${data.month}-`)) return selection;
		if (date?.startsWith(`${data.month}-`) && date.length === 10 && isValid(parseISO(date)))
			return date;
		return data.today.startsWith(`${data.month}-`) ? data.today : data.monthDate;
	});

	$effect.pre(() => {
		const source = data.schedule;
		completed = undefined;
		if (!source) return;
		let active = true;
		void source.then((read) => {
			if (active && read && read.identity === appIdentityKey()) completed = { source, read };
		});
		return () => {
			active = false;
		};
	});

	function selectDate(date: string) {
		const month = date.slice(0, 7);
		if (month === data.month) {
			selection = date;
			return;
		}
		selection = '';
		const url = new URL(page.url.href);
		url.searchParams.set('month', month);
		url.searchParams.set('date', date);
		void goto(resolve(`/staff/academic/supervision/calendar?${url.searchParams}`), {
			keepFocus: true,
			noScroll: true
		});
	}
</script>

<PageShell title={data.title}>
	{#if !data.schedule}
		<PageState title="ไม่พบปีการศึกษาสำหรับแสดงปฏิทิน" />
	{:else}
		<SupervisionCalendarSchedule
			view="staff"
			bookings={entries}
			{cycles}
			monthDate={data.monthDate}
			{selectedDate}
			ondatechange={selectDate}
			loading={!result}
			error={result?.ok === false}
			onretry={() => void invalidate('school:supervision-calendar')}
		/>
	{/if}
</PageShell>
