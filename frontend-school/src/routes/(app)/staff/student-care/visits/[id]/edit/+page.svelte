<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { careIdentity, careManagePermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careVisit, careStaff } from '#lib/api/student-care.js';
	import CareVisitForm from '#lib/components/student-care/CareVisitForm.svelte';
	let { data }: PageProps = $props();
	async function retry(signal: AbortSignal) {
		const record = await careVisit(data.id, { signal });
		const staff = await careStaff({ signal });
		return { record, staff, students: [] };
	}
</script>

<PageShell title="บันทึกเยี่ยมบ้าน" backHref={careLink('/visits', data.term)}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careManagePermissions}
			{retry}
			variant="form"
			>{#snippet children(options)}<CareVisitForm
					initial={options.record}
					students={options.students}
					staff={options.staff}
					term={options.record?.visit.academicTermId ?? data.term}
					date={data.date}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
