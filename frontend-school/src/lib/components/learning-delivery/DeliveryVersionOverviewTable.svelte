<script lang="ts">
	import { resolve } from '$app/paths';
	import type { DeliveryVersion } from '#lib/api/learning-delivery.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		version,
		initialKind = 'all'
	}: { version: DeliveryVersion; initialKind?: 'all' | 'activity' } = $props();
	let search = $state('');
	let kind = $derived<'all' | 'course' | 'activity'>(initialKind);
	const items = $derived(
		version.snapshot.offerings.filter(
			(item) =>
				(kind === 'all' || item.kind === kind) &&
				`${item.code} ${item.name}`
					.toLocaleLowerCase('th-TH')
					.includes(search.trim().toLocaleLowerCase('th-TH'))
		)
	);
	function detailUrl(id: string) {
		const query = new URLSearchParams({
			academicYearId: version.academicYearId,
			academicTermId: version.academicTermId,
			deliveryVersionId: version.id
		});
		return resolve(`staff/academic/delivery/${id}?${query}`);
	}
</script>

<div class="flex flex-col gap-3 border-b p-4 sm:flex-row">
	<Input
		bind:value={search}
		aria-label="ค้นหารายการเปิดสอน"
		placeholder="ค้นหารหัสหรือชื่อรายวิชาและกิจกรรม"
	/>
	<Select.Root type="single" bind:value={kind}>
		<Select.Trigger class="w-full sm:w-48" aria-label="กรองประเภทรายการเปิดสอน"
			>{kind === 'all' ? 'ทุกประเภท' : kind === 'course' ? 'รายวิชา' : 'กิจกรรม'}</Select.Trigger
		>
		<Select.Content
			><Select.Item value="all">ทุกประเภท</Select.Item><Select.Item value="course"
				>รายวิชา</Select.Item
			><Select.Item value="activity">กิจกรรม</Select.Item></Select.Content
		>
	</Select.Root>
</div>
{#if items.length === 0}
	<p class="p-6 text-center text-sm text-muted-foreground">ไม่พบรายการเปิดสอนที่ตรงกับตัวกรอง</p>
{:else}
	<div class="overflow-x-auto">
		<Table.Root>
			<Table.Header
				><Table.Row
					><Table.Head>รายการเปิดสอน</Table.Head><Table.Head>คาบ/สัปดาห์</Table.Head><Table.Head
						>กลุ่มเรียน</Table.Head
					><Table.Head>ครูหลัก</Table.Head><Table.Head
						><span class="sr-only">รายละเอียด</span></Table.Head
					></Table.Row
				></Table.Header
			>
			<Table.Body>
				{#each items as item (item.id)}
					<Table.Row>
						<Table.Cell
							><p class="font-medium">{item.code} · {item.name}</p>
							<p class="text-xs text-muted-foreground">
								{item.kind === 'course' ? 'รายวิชา' : 'กิจกรรม'}
							</p></Table.Cell
						>
						<Table.Cell>{item.weeklyPeriodTarget}</Table.Cell><Table.Cell
							>{item.groups.length}</Table.Cell
						>
						<Table.Cell
							>{item.groups.filter((group) =>
								group.teachers.some((teacher) => teacher.role === 'primary')
							).length}/{item.groups.length} กลุ่ม</Table.Cell
						>
						<Table.Cell
							><Button variant="outline" size="sm" href={detailUrl(item.id)}>รายละเอียด</Button
							></Table.Cell
						>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
{/if}
