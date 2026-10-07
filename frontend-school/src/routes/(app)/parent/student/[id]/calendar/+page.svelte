<script lang="ts">
	import type { PageProps } from './$types';
	import CalendarViewer from '#lib/components/calendar/CalendarViewer.svelte';
	let { data }: PageProps = $props();
</script>

<CalendarViewer
	title={data.title}
	requestHref={data.requestHref}
	requestKey={data.requestKey}
	month={data.month}
	records={data.records}
	studentId={data.studentId}
/>
