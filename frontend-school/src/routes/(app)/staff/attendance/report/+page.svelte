<script lang="ts">
	import { PageShell } from '#lib/components/app-layout/index.js';
	import AttendanceReportPanel from '#lib/features/attendance/AttendanceReportPanel.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
</script>

<PageShell title={data.title}>
	{#if !data.term}<p>กรุณาเลือกภาคเรียนจากแถบด้านบน</p>
	{:else}{#await data.initial}<p role="status">
				กำลังโหลด…
			</p>{:then result}{#if result?.ok}{#key data.term + data.date}<AttendanceReportPanel
						term={data.term}
						date={data.date}
						initial={result.data}
					/>{/key}{:else}<p role="alert">{result?.error}</p>{/if}{/await}{/if}
</PageShell>
