<script lang="ts">
	import { UserRound } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { careAssignments } from '#lib/api/student-care.js';
	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareAssignmentList from '#lib/components/student-care/CareAssignmentList.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับประวัติการดูแลรายบุคคล"
	icon={UserRound}
	title="ประวัติแบบประเมินรายบุคคล"
	backHref={`/staff/student-care/students/${data.id}`}
	description="ทุกภาคเรียนในขอบเขตที่คุณรับผิดชอบ"
	>{#key data.url}<CareFilters
			url={data.url}
			kind="assignments"
		/>{/key}{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careReadPermissions}
			retry={(signal) => careAssignments(data.query, { signal })}
			>{#snippet children(items)}<CareAssignmentList
					{items}
					filtered={!!data.query.status || !!data.query.afterId}
				/><CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
