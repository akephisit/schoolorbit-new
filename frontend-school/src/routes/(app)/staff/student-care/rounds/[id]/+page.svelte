<script lang="ts">
	import { ClipboardList } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CarePager from '#lib/components/student-care/CarePager.svelte';
	import CareSection from '#lib/components/student-care/CareSection.svelte';
	import { careRound, careAssignments } from '#lib/api/student-care.js';
	import CareRoundDetail from '#lib/components/student-care/CareRoundDetail.svelte';
	import CareAssignmentList from '#lib/components/student-care/CareAssignmentList.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับรอบประเมิน"
	icon={ClipboardList}
	title="รายละเอียดรอบประเมิน"
	backHref={careLink('/rounds', data.term)}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.round}
			permissions={careReadPermissions}
			retry={(signal) => careRound(data.id, { signal })}
			variant="detail"
			>{#snippet children(initial)}<CareRoundDetail {initial} />{/snippet}</CareRegion
		><CareSection title="แบบประเมินที่มอบหมาย"
			><CareRegion
				initial={data.assignments}
				permissions={careReadPermissions}
				retry={(signal) => careAssignments(data.query, { signal })}
				>{#snippet children(items)}<CareAssignmentList
						{items}
						term={data.term}
						allowManagement
					/><CarePager
						url={data.url}
						lastId={items.at(-1)?.id}
						count={items.length}
					/>{/snippet}</CareRegion
			></CareSection
		>{/key}</PageShell
>
