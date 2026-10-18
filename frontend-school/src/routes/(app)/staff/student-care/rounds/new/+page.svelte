<script lang="ts">
	import { ClipboardList } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { PageState } from '#lib/components/app-state/index.js';

	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careInstruments } from '#lib/api/student-care.js';
	import CareRoundForm from '#lib/components/student-care/CareRoundForm.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับรอบประเมิน"
	icon={ClipboardList}
	title="สร้างรอบประเมิน"
	backHref={careLink('/rounds', data.term)}
	>{#if data.term}{#key $careIdentity + data.term}<CareRegion
				initial={data.initial}
				permissions={[PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL]}
				retry={(signal) => careInstruments({ limit: 100 }, { signal })}
				variant="form"
				>{#snippet children(instruments)}<CareRoundForm
						{instruments}
						term={data.term!}
						date={data.date}
					/>{/snippet}</CareRegion
			>{/key}{:else}<PageState
			title="เลือกภาคเรียน"
			description="เลือกภาคเรียนจากแถบด้านบนเพื่อสร้างรอบ"
		/>{/if}</PageShell
>
