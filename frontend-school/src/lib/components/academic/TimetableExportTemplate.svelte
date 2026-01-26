<script lang="ts">
	import type { TimetableEntry } from '$lib/api/timetable';

	export let title = '';
	export let subTitle = '';
	// periods should use any type if the strict type isn't shared, but better to be safe
	export let periods: any[] = [];
	export let timetableEntries: TimetableEntry[] = [];
	
	// Helper for Days
	const DAYS = [
		{ value: 'MON', label: 'จันทร์', color: '#FEF9C3' }, // Yellow-50
		{ value: 'TUE', label: 'อังคาร', color: '#FCE7F3' }, // Pink-50
		{ value: 'WED', label: 'พุธ', color: '#DCFCE7' },    // Green-50
		{ value: 'THU', label: 'พฤหัสฯ', color: '#FFEDD5' }, // Orange-50
		{ value: 'FRI', label: 'ศุกร์', color: '#DBEAFE' },   // Blue-50
		{ value: 'SAT', label: 'เสาร์', color: '#F3F4F6' },
		{ value: 'SUN', label: 'อาทิตย์', color: '#F3F4F6' }
	];

	function formatTime(timeStr: string) {
		if (!timeStr) return '';
		return timeStr.substring(0, 5);
	}

	function getEntry(day: string, periodId: string) {
		return timetableEntries.find(
			(e) => e.day_of_week === day && e.period_id === periodId && e.is_active
		);
	}
</script>

<!-- 
    A4 Landscape Width approx 297mm. 
    html2pdf usually works best with pixels roughly screen size ~1123px width for full quality.
    We set a fixed width container to ensure consistent "Print" layout regardless of screen size.
-->
<div
	id="timetable-print-template"
	class="bg-white text-black font-sans p-8"
	style="width: 1200px; margin: 0 auto;"
>
	<!-- Header -->
	<div class="text-center mb-6">
		<h1 class="text-3xl font-bold mb-2 text-primary">{title}</h1>
		{#if subTitle}
			<p class="text-xl text-gray-600">{subTitle}</p>
		{/if}
	</div>

	<!-- Grid -->
	<table class="w-full border-collapse border border-gray-800">
		<thead>
			<tr class="bg-gray-100">
				<th class="border border-gray-400 p-2 w-28 text-center text-lg">วัน / เวลา</th>
				{#each periods as p}
					<th class="border border-gray-400 p-2 text-center min-w-[80px]">
						<div class="text-lg font-bold">คาบที่ {p.order_index}</div>
						<div class="text-sm font-normal text-gray-600">
							{formatTime(p.start_time)} - {formatTime(p.end_time)}
						</div>
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each DAYS.slice(0, 5) as day}
				<!-- Usually Mon-Fri. If weekend has classes, expand logic -->
				<tr class="h-28">
					<!-- Day Header -->
					<td
						class="border border-gray-400 p-4 font-bold text-center text-xl relative"
						style="background-color: {day.color};"
					>
						{day.label}
					</td>

					<!-- Slots -->
					{#each periods as p}
						{@const entry = getEntry(day.value, p.id)}
						<td class="border border-gray-400 p-1 text-center align-middle relative bg-white">
							{#if entry}
								<div
									class="flex flex-col items-center justify-center gap-1 w-full h-full p-1 rounded"
								>
									<!-- Course Code & Name -->
									<div class="font-bold text-lg text-blue-900 leading-tight">
										{entry.subject_code || ''}
									</div>
									<div class="text-base text-gray-800 line-clamp-2 px-1">
										{entry.subject_name_th || entry.subject_name_en || 'วิชา'}
									</div>

									<!-- Detail (Room or Instructor depending on View) -->
									{#if entry.room_code || entry.classroom_name}
										<div
											class="mt-1 text-sm bg-gray-100 px-2 py-0.5 rounded-full border border-gray-300"
										>
											{#if entry.room_code}
												ห้อง {entry.room_code}
											{:else}
												{entry.classroom_name || ''}
											{/if}
										</div>
									{/if}
								</div>
							{:else}
								<!-- Empty Slot -->
								<div class="text-gray-300 text-sm"></div>
							{/if}
						</td>
					{/each}
				</tr>
			{/each}
		</tbody>
	</table>

	<!-- Footer / Metadata if needed -->
	<div class="mt-4 flex justify-between text-xs text-gray-400">
		<div>ข้อมูล ณ วันที่ {new Date().toLocaleDateString('th-TH')}</div>
		<div>SchoolOrbit TimeTable</div>
	</div>
</div>
