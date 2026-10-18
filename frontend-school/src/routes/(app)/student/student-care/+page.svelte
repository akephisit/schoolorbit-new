<script lang="ts">
	import { Inbox } from '@lucide/svelte';
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';

	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { ownCareAssignments } from '#lib/features/student-care/own-route.js';
	import CareAssignmentList from '#lib/components/student-care/CareAssignmentList.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	icon={Inbox}
	title="แบบประเมินของฉัน"
	description="ตอบแบบประเมินที่โรงเรียนมอบหมาย บันทึกร่างแล้วกลับมาทำต่อได้"
	>{#key data.url}<CareFilters
			url={data.url}
			kind="assignments"
		/>{/key}{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careReadPermissions}
			retry={(signal) => ownCareAssignments(data.query, { signal })}
			>{#snippet children(items)}<CareAssignmentList
					{items}
					audience="own"
					filtered={!!data.query.status || !!data.query.afterId}
				/><CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
