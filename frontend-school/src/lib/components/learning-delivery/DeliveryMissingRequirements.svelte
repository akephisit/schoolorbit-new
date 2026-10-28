<script lang="ts">
	import type { HomeroomDeliveryWorkspace } from '#lib/api/learning-delivery.js';
	import type { DeliveryAlignmentFacts } from '#lib/academic/homeroom-delivery.js';
	import type { CurriculumPreparationFocus } from '#lib/academic/synchronized-activity-delivery.js';
	import { deliveryChangePage } from '#lib/academic/learning-delivery-change-list.js';
	import type { DeliveryOverviewState } from '#lib/navigation/learning-delivery.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Plus } from '@lucide/svelte';

	let {
		workspace,
		facts,
		loading,
		error,
		canManage,
		preparing,
		filters,
		showFilters,
		onFiltersChange,
		onRetry,
		onPrepare
	}: {
		workspace: HomeroomDeliveryWorkspace | null;
		facts: DeliveryAlignmentFacts | null;
		loading: boolean;
		error: string;
		canManage: boolean;
		preparing: boolean;
		filters: Pick<DeliveryOverviewState, 'kind' | 'offeringSearch' | 'missingPage'>;
		showFilters: boolean;
		onFiltersChange: (patch: Partial<DeliveryOverviewState>) => void;
		onRetry: () => Promise<void>;
		onPrepare: (focus: CurriculumPreparationFocus) => Promise<void>;
	} = $props();
	const query = $derived(filters.offeringSearch.trim().toLocaleLowerCase('th-TH'));
	const missing = $derived(
		deliveryChangePage(
			(facts?.missingRequirements ?? []).filter(
				(item) =>
					(filters.kind === 'all' || filters.kind === item.resourceKind) &&
					`${item.resourceKind === 'course' ? item.code : ''} ${item.name}`
						.toLocaleLowerCase('th-TH')
						.includes(query)
			),
			filters.missingPage
		)
	);
</script>

<section
	class="mt-4 overflow-hidden rounded-2xl border bg-card"
	aria-label="ตามหลักสูตรที่ยังไม่เปิดสอน"
	aria-busy={loading}
>
	<div class="space-y-2 border-b bg-muted/20 p-4">
		<h2 class="font-semibold">ตามหลักสูตรที่ยังไม่เปิดสอน</h2>
		<p class="text-sm text-muted-foreground">
			ข้อกำหนดที่ยังขาดของทุกห้องในรุ่นที่เลือก · รวมทุกกลุ่มสาระ
			ตัวกรองกลุ่มสาระไม่ใช้กับรายการส่วนนี้ ใช้ประเภทและคำค้นเดียวกับรายการเปิดสอน
		</p>
		{#if showFilters}<div class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
				<Input
					value={filters.offeringSearch}
					aria-label="ค้นหารายการเปิดสอน"
					placeholder="ค้นหารหัสหรือชื่อรายวิชาและกิจกรรม"
					oninput={(event) => onFiltersChange({ offeringSearch: event.currentTarget.value })}
				/>
				<Select.Root
					type="single"
					value={filters.kind}
					onValueChange={(kind) => {
						if (kind === 'all' || kind === 'course' || kind === 'activity')
							onFiltersChange({ kind });
					}}
				>
					<Select.Trigger aria-label="กรองประเภทรายการเปิดสอน"
						>{filters.kind === 'course'
							? 'รายวิชา'
							: filters.kind === 'activity'
								? 'กิจกรรม'
								: 'ทุกประเภท'}</Select.Trigger
					>
					<Select.Content
						><Select.Item value="all">ทุกประเภท</Select.Item><Select.Item value="course"
							>รายวิชา</Select.Item
						><Select.Item value="activity">กิจกรรม</Select.Item></Select.Content
					>
				</Select.Root>
			</div>{/if}
	</div>
	{#if loading && !facts}<div class="p-4"><PageSkeleton variant="table" rows={3} /></div>{/if}
	{#if error}<div class="space-y-2 p-4">
			<p role="alert" class="text-sm text-destructive">{error}</p>
			<Button variant="outline" onclick={() => onRetry()}>ลองโหลดข้อกำหนดอีกครั้ง</Button>
		</div>{/if}
	{#if facts && workspace}
		{#if facts.prerequisites.length}<p class="px-4 pt-4 text-sm text-muted-foreground">
				มี {facts.prerequisites.length} ห้องที่ยังขาดข้อมูลโครงสร้าง จึงยังตรวจข้อกำหนดของห้องเหล่านั้นไม่ได้
			</p>{/if}
		{#if missing.items.length}
			<div class="overflow-x-auto">
				<Table.Root>
					<Table.Header
						><Table.Row
							><Table.Head>ตามหลักสูตร</Table.Head><Table.Head>ห้องที่ยังขาด</Table.Head><Table.Head
								>จัดการ</Table.Head
							></Table.Row
						></Table.Header
					>
					<Table.Body
						>{#each missing.items as item (item.key)}<Table.Row
								data-testid="delivery-inline-missing-requirement"
							>
								<Table.Cell
									><p class="font-medium">
										{item.resourceKind === 'course' ? `${item.code} · ` : ''}{item.name}
									</p>
									<p class="text-xs text-muted-foreground">
										{item.resourceKind === 'course' ? 'รายวิชา' : 'กิจกรรม'} · ยังไม่เปิดสอนตามหลักสูตร
									</p></Table.Cell
								>
								<Table.Cell
									><p class="text-sm">{item.rooms.map((room) => room.name).join(', ')}</p>
									<p class="text-xs text-muted-foreground">
										รวม {item.rooms.length} ห้อง
									</p></Table.Cell
								>
								<Table.Cell>
									{#if canManage && workspace.deliveryVersionId && !error}<Button
											type="button"
											size="sm"
											variant="outline"
											disabled={preparing || loading}
											onclick={() => onPrepare(item)}
											><Plus class="size-3.5" />เพิ่มการจัดการเรียนการสอน</Button
										>
									{:else if canManage && !workspace.deliveryVersionId}<span
											class="text-xs text-muted-foreground">สร้างรุ่นเปิดสอนก่อนเพิ่มรายการ</span
										>{/if}
								</Table.Cell>
							</Table.Row>{/each}</Table.Body
					>
				</Table.Root>
			</div>
			<div
				class="flex flex-wrap items-center justify-end gap-2 border-t p-3 text-sm"
				aria-label="หน้าข้อกำหนดที่ยังไม่เปิดสอน"
			>
				<span>{missing.total} รายการ · หน้า {missing.page + 1}/{missing.pageCount}</span>
				<Button
					variant="outline"
					size="sm"
					disabled={missing.page === 0}
					onclick={() => onFiltersChange({ missingPage: missing.page - 1 })}
					>ข้อกำหนดก่อนหน้า</Button
				>
				<Button
					variant="outline"
					size="sm"
					disabled={missing.page + 1 === missing.pageCount}
					onclick={() => onFiltersChange({ missingPage: missing.page + 1 })}>ข้อกำหนดถัดไป</Button
				>
			</div>
		{:else}<p class="p-4 text-sm text-muted-foreground">
				{query || filters.kind !== 'all'
					? 'ไม่พบข้อกำหนดที่ยังขาดตามตัวกรอง'
					: facts.prerequisites.length
						? 'ยังตรวจข้อกำหนดของห้องที่ขาดโครงสร้างไม่ได้'
						: 'ไม่พบข้อกำหนดที่ยังขาดในข้อมูลที่ตรวจได้'}
			</p>{/if}
	{:else if !loading && !error}<p class="p-4 text-sm text-muted-foreground">
			ยังไม่มีข้อมูลข้อกำหนดที่ตรงกับปี ภาคเรียน และรุ่นที่เลือก
		</p>{/if}
</section>
