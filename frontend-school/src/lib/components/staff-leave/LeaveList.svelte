<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { LeaveKind, LeaveListQuery, LeaveRequestPage } from '#lib/api/staff-leave.js';
	import { listLeaveRequests } from '#lib/api/staff-leave.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import {
		leaveHome,
		leaveDate,
		leaveKindLabels,
		leaveAmount,
		leaveStatusLabels,
		leaveStatusClass,
		leaveStatusVariant
	} from '#lib/staff-leave/presentation.js';
	import { leaveListDestination, leavePageSize } from '#lib/staff-leave/filters.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, TableSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { ChevronDown, Plus } from '@lucide/svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import Choice from './Choice.svelte';
	import RequestEditor from './RequestEditor.svelte';
	let {
		source,
		kind,
		query
	}: {
		source: Promise<RouteLoadResult<LeaveRequestPage | null>>;
		kind: LeaveKind;
		query: LeaveListQuery;
	} = $props();
	const region = new LeaveRegion<LeaveRequestPage | null>();
	let creating = $state(false);
	const home = $derived(leaveHome(kind));
	const offset = $derived(query.offset ?? 0);
	$effect.pre(() => {
		region.consume(source);
	});
	onDestroy(() => region.dispose());
	function filter(status: string) {
		void goto(leaveListDestination(home, status));
	}
</script>

<PageShell
	contentClass="space-y-4"
	title={kind === 'exit' ? 'ขออนุญาตออกนอกโรงเรียน' : 'การลา'}
	description="คำขอของคุณและรายการที่ได้รับสิทธิ์ให้พิจารณา"
>
	{#snippet actions()}
		{#if $can.hasAny(PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN, PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL, PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL)}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					>{#snippet child({ props })}<Button {...props} variant="outline"
							>เครื่องมือ<ChevronDown class="size-4" /></Button
						>{/snippet}</DropdownMenu.Trigger
				>
				<DropdownMenu.Content align="end">
					{#if $can.hasAny(PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN, PERMISSIONS.STAFF_LEAVE_DOWNLOAD_SCHOOL)}<DropdownMenu.Item
							onclick={() => goto('/staff/leave/reports')}>สรุปการใช้สิทธิ์</DropdownMenu.Item
						>{/if}
					{#if $can.has(PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL)}<DropdownMenu.Item
							onclick={() => goto('/staff/leave/settings')}>ตั้งค่าประเภทและเกณฑ์</DropdownMenu.Item
						>{/if}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
		{#if $can.has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN)}<Button onclick={() => (creating = true)}
				><Plus class="size-4" />สร้างคำขอ{leaveKindLabels[kind]}</Button
			>{/if}
	{/snippet}
	<div class="flex flex-wrap items-end justify-between gap-3 border-b pb-3">
		<div class="w-full sm:w-56">
			<Choice
				id="leave-status-filter"
				label="สถานะ"
				value={query.status ?? 'all'}
				onchange={(value) => filter(value === 'all' ? '' : value)}
				options={[
					{ value: 'all', label: 'ทั้งหมด' },
					...Object.entries(leaveStatusLabels).map(([value, label]) => ({ value, label }))
				]}
			/>
		</div>
	</div>
	<section data-testid="leave-requests" aria-busy={region.loading}>
		{#if region.error}<PageState
				variant="error"
				title="โหลดคำขอไม่สำเร็จ"
				description={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listLeaveRequests({ signal, query }))}
			/>{/if}
		{#if region.loading && !region.value}<div role="status" aria-label="กำลังโหลดคำขอ">
				<TableSkeleton columns={5} />
			</div>
		{:else if region.value?.items.length}
			<div class="hidden overflow-hidden rounded-lg border bg-card md:block">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>ประเภท / ผู้ขอ</Table.Head><Table.Head>ช่วงวันที่</Table.Head><Table.Head
								>จำนวน</Table.Head
							><Table.Head>สถานะ</Table.Head><Table.Head>รายละเอียด</Table.Head></Table.Row
						></Table.Header
					>
					<Table.Body
						>{#each region.value.items as request (request.id)}<Table.Row>
								<Table.Cell
									><p class="font-medium">{request.typeName}</p>
									<p class="text-sm text-muted-foreground">{request.ownerName}</p></Table.Cell
								>
								<Table.Cell
									><p>
										{leaveDate(request.startDate)}{request.endDate !== request.startDate
											? ` – ${leaveDate(request.endDate)}`
											: ''}
									</p>
									{#if request.startTime}<p class="text-sm text-muted-foreground">
											{request.startTime.slice(0, 5)} – {request.endTime?.slice(0, 5)}
										</p>{/if}</Table.Cell
								>
								<Table.Cell>{leaveAmount(request.amount, request.unit)}</Table.Cell>
								<Table.Cell
									><Badge
										variant={leaveStatusVariant(request.status)}
										class={leaveStatusClass(request.status)}
										>{leaveStatusLabels[request.status]}</Badge
									></Table.Cell
								>
								<Table.Cell
									><Button
										variant="outline"
										size="sm"
										href={`/staff/leave/requests/${request.id}?returnTo=${encodeURIComponent(`${page.url.pathname}${page.url.search}`)}`}
										>เปิดคำขอ</Button
									></Table.Cell
								>
							</Table.Row>{/each}</Table.Body
					>
				</Table.Root>
			</div>
			<ul class="divide-y rounded-lg border bg-card md:hidden" aria-label="รายการคำขอ">
				{#each region.value.items as request (request.id)}
					<li class="space-y-3 p-3">
						<div class="flex items-start justify-between gap-2">
							<div class="min-w-0">
								<p class="break-words font-medium">{request.typeName}</p>
								<p class="break-words text-sm text-muted-foreground">{request.ownerName}</p>
							</div>
							<Badge
								variant={leaveStatusVariant(request.status)}
								class={leaveStatusClass(request.status)}>{leaveStatusLabels[request.status]}</Badge
							>
						</div>
						<div class="flex items-end justify-between gap-3">
							<div class="text-sm">
								<p>
									{leaveDate(request.startDate)}{request.endDate !== request.startDate
										? ` – ${leaveDate(request.endDate)}`
										: ''}
								</p>
								{#if request.startTime}<p class="text-muted-foreground">
										{request.startTime.slice(0, 5)} – {request.endTime?.slice(0, 5)}
									</p>{/if}
								<p class="mt-1 font-medium">{leaveAmount(request.amount, request.unit)}</p>
							</div>
							<Button
								variant="outline"
								size="sm"
								href={`/staff/leave/requests/${request.id}?returnTo=${encodeURIComponent(`${page.url.pathname}${page.url.search}`)}`}
								>เปิดคำขอ</Button
							>
						</div>
					</li>
				{/each}
			</ul>
			<div class="mt-3 flex flex-wrap items-center justify-between gap-3">
				<p class="text-sm text-muted-foreground">
					รายการ {offset + 1} – {offset + region.value.items.length}
				</p>
				<div class="flex gap-2">
					<Button
						variant="outline"
						disabled={offset === 0}
						href={offset
							? leaveListDestination(home, query.status ?? '', Math.max(0, offset - leavePageSize))
							: undefined}>ก่อนหน้า</Button
					><Button
						variant="outline"
						disabled={!region.value.hasMore}
						href={region.value.hasMore
							? leaveListDestination(home, query.status ?? '', offset + leavePageSize)
							: undefined}>ถัดไป</Button
					>
				</div>
			</div>
		{:else if !region.error}<PageState
				title={query.status ? 'ไม่พบคำขอตามสถานะที่เลือก' : 'ยังไม่มีคำขอ'}
				description={query.status
					? 'เปลี่ยนสถานะเพื่อดูรายการอื่น'
					: 'เริ่มสร้างคำขอเมื่อโรงเรียนตั้งค่านโยบายที่ใช้กับคุณแล้ว'}
			/>{/if}
	</section>
</PageShell>

<Dialog.Root bind:open={creating}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>สร้างคำขอ{leaveKindLabels[kind]}</Dialog.Title><Dialog.Description
				>บันทึกฉบับร่างก่อนแนบหลักฐานและส่งพิจารณา</Dialog.Description
			></Dialog.Header
		>
		{#if creating && $can.has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN)}<RequestEditor
				{kind}
				oncancel={() => (creating = false)}
				onsaved={(detail) => {
					creating = false;
					void goto(
						`/staff/leave/requests/${detail.request.id}?returnTo=${encodeURIComponent(`${page.url.pathname}${page.url.search}`)}`
					);
				}}
			/>{/if}
	</Dialog.Content></Dialog.Root
>
