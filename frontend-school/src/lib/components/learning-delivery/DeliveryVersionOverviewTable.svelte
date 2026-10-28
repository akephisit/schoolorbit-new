<script lang="ts">
	import DeliveryDraftResourceRemoval from './DeliveryDraftResourceRemoval.svelte';
	import type { AcademicTermChangeSet, DeliveryVersion } from '#lib/api/learning-delivery.js';
	import {
		deliveryOfferingHref,
		type DeliveryOverviewState
	} from '#lib/navigation/learning-delivery.js';
	import {
		deliveryOfferingLabel,
		deliverySubjectGroupOptions,
		filterDeliveryOfferings
	} from '#lib/academic/learning-delivery-presentation.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let {
		version,
		filters,
		returnTo,
		onFiltersChange,
		draftChangeSetId = null,
		onDraftResourceRemoved
	}: {
		version: DeliveryVersion;
		filters: Pick<DeliveryOverviewState, 'kind' | 'offeringSearch' | 'subjectGroupId'>;
		returnTo: string;
		onFiltersChange: (patch: Partial<DeliveryOverviewState>) => void;
		draftChangeSetId?: string | null;
		onDraftResourceRemoved?: (updated: AcademicTermChangeSet) => Promise<void>;
	} = $props();
	const groups = $derived(deliverySubjectGroupOptions(version));
	const items = $derived(filterDeliveryOfferings(version, filters));
	function detailUrl(id: string) {
		return deliveryOfferingHref(
			id,
			{
				academicYearId: version.academicYearId,
				academicTermId: version.academicTermId,
				deliveryVersionId: version.id
			},
			returnTo
		);
	}
</script>

<div class="grid gap-3 border-b p-4 sm:grid-cols-[minmax(0,1fr)_auto_auto]">
	<Input
		value={filters.offeringSearch}
		oninput={(event) => onFiltersChange({ offeringSearch: event.currentTarget.value })}
		aria-label="ค้นหารายการเปิดสอน"
		placeholder="ค้นหารหัสหรือชื่อรายวิชาและกิจกรรม"
	/>
	<Select.Root
		type="single"
		value={filters.kind}
		onValueChange={(kind) => {
			if (kind === 'all' || kind === 'course' || kind === 'activity') onFiltersChange({ kind });
		}}
	>
		<Select.Trigger class="w-full sm:w-48" aria-label="กรองประเภทรายการเปิดสอน"
			>{filters.kind === 'all'
				? 'ทุกประเภท'
				: filters.kind === 'course'
					? 'รายวิชา'
					: 'กิจกรรม'}</Select.Trigger
		>
		<Select.Content
			><Select.Item value="all">ทุกประเภท</Select.Item><Select.Item value="course"
				>รายวิชา</Select.Item
			><Select.Item value="activity">กิจกรรม</Select.Item></Select.Content
		>
	</Select.Root>
	<Select.Root
		type="single"
		value={filters.subjectGroupId}
		onValueChange={(subjectGroupId) => onFiltersChange({ subjectGroupId })}
	>
		<Select.Trigger class="w-full sm:w-56" aria-label="กรองกลุ่มสาระรายการเปิดสอน">
			{filters.subjectGroupId === 'all'
				? 'ทุกกลุ่มสาระ'
				: (groups.find((group) => group.id === filters.subjectGroupId)?.name ??
					'ไม่พบกลุ่มสาระที่เลือกในรุ่นนี้')}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">ทุกกลุ่มสาระ</Select.Item>
			{#each groups as group (group.id)}<Select.Item value={group.id}>{group.name}</Select.Item
				>{/each}
		</Select.Content>
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
							><p class="font-medium">{deliveryOfferingLabel(item)}</p>
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
							><div class="flex flex-wrap justify-end gap-1">
								<Button
									variant="outline"
									size="sm"
									href={detailUrl(item.id)}
									data-sveltekit-preload-data="tap">รายละเอียด</Button
								>
								{#if version.status === 'draft' && draftChangeSetId && onDraftResourceRemoved}<DeliveryDraftResourceRemoval
										changeSetId={draftChangeSetId}
										versionId={version.id}
										offeringId={item.id}
										label={deliveryOfferingLabel(item)}
										canManage={true}
										onRemoved={onDraftResourceRemoved}
									/>{/if}
							</div></Table.Cell
						>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
{/if}
