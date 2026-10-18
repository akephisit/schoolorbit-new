<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { PageState } from '#lib/components/app-state/index.js';

	import { careIdentity, careManagePermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import { careStaff, careStudents } from '#lib/api/student-care.js';
	import CareVisitForm from '#lib/components/student-care/CareVisitForm.svelte';
	let { data }: PageProps = $props();
	async function retry(signal: AbortSignal) {
		const [staff, students] = await Promise.all([
			careStaff({ signal }),
			careStudents({ academicTermId: data.term, date: data.date, limit: 50 }, { signal })
		]);
		return { record: undefined, staff, students };
	}
</script>

<PageShell title="วางแผนเยี่ยมบ้าน" backHref={careLink('/visits', data.term)}
	>{#if data.term}{#key $careIdentity + data.term}<CareRegion
				initial={data.initial}
				permissions={careManagePermissions}
				{retry}
				variant="form"
				>{#snippet children(options)}<CareVisitForm
						initial={options.record}
						students={options.students}
						staff={options.staff}
						term={data.term}
						date={data.date}
					/>{/snippet}</CareRegion
			>{/key}{:else}<PageState
			title="เลือกภาคเรียน"
			description="เลือกภาคเรียนจากแถบด้านบน"
		/>{/if}</PageShell
>
