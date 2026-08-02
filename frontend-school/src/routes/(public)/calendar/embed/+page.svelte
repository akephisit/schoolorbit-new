<script lang="ts">
	import PublicCalendarView from '$lib/components/calendar/PublicCalendarView.svelte';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.title}</title>
</svelte:head>

<PublicCalendarView mode="embed" />
