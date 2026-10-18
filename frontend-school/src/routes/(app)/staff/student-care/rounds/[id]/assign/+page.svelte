<script lang="ts">
	import { ClipboardList } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { careIdentity, careManagePermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careRound, careInstrument, careStudents, careStaff } from '#lib/api/student-care.js';
	import CareAssignmentForm from '#lib/components/student-care/CareAssignmentForm.svelte';
	let { data }: PageProps = $props();
	async function retry(signal: AbortSignal) {
		const round = await careRound(data.id, { signal });
		const [instruments, students, staff] = await Promise.all([
			Promise.all(round.instrumentVersionIds.map((id) => careInstrument(id, { signal }))),
			careStudents(
				{ academicTermId: round.academicTermId, date: round.rosterDate, limit: 50 },
				{ signal }
			),
			careStaff({ signal })
		]);
		return { round, instruments, students, staff };
	}
</script>

<PageShell
	backLabel="กลับรายละเอียดรอบประเมิน"
	icon={ClipboardList}
	title="มอบหมายแบบประเมิน"
	backHref={careLink(`/rounds/${data.id}`)}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careManagePermissions}
			{retry}
			variant="form"
			>{#snippet children(options)}<CareAssignmentForm {...options} />{/snippet}</CareRegion
		>{/key}</PageShell
>
