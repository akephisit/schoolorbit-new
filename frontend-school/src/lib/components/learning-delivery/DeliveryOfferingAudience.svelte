<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		getHomeroomDeliveryWorkspace,
		type AcademicTermChangeSet,
		type DeliveryVersion,
		type HomeroomDeliveryWorkspace
	} from '#lib/api/learning-delivery.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import DeliveryDraftResourceRemoval from './DeliveryDraftResourceRemoval.svelte';

	type WorkspaceLoad = {
		academicYearId: string;
		academicTermId: string;
		deliveryVersionId: string;
		result: RouteLoadResult<HomeroomDeliveryWorkspace>;
	};
	let {
		version,
		offering,
		initialWorkspace,
		canManage,
		changeSetId = null,
		onRemoved
	}: {
		version: DeliveryVersion;
		offering: DeliveryVersion['snapshot']['offerings'][number];
		initialWorkspace: Promise<WorkspaceLoad | null>;
		canManage: boolean;
		changeSetId?: string | null;
		onRemoved: (updated: AcademicTermChangeSet) => Promise<void>;
	} = $props();
	let workspace = $state.raw<HomeroomDeliveryWorkspace | null>(null);
	let loading = $state(true);
	let error = $state('');
	let disposed = false;
	const request = new LatestRequest();
	const context = $derived(
		`${version.academicYearId}:${version.academicTermId}:${version.id}:${offering.id}`
	);
	const hasAudience = $derived(offering.homeroomIds.length > 0);
	const rooms = $derived(
		offering.homeroomIds.map((id) => ({
			id,
			name: workspace?.homerooms.find((room) => room.homeroom.id === id)?.homeroom.name ?? null,
			groupCount: offering.groups.filter((group) => group.homeroomIds.includes(id)).length
		}))
	);
	function current(owner: string, revision: number) {
		return !disposed && owner === context && request.isCurrent(revision);
	}
	async function load(initial?: Promise<WorkspaceLoad | null>) {
		const owner = context,
			yearId = version.academicYearId,
			termId = version.academicTermId,
			versionId = version.id;
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const seeded = initial ? await initial : null;
			if (!current(owner, ticket.revision)) return;
			let loaded: HomeroomDeliveryWorkspace;
			if (
				seeded &&
				seeded.academicYearId === yearId &&
				seeded.academicTermId === termId &&
				seeded.deliveryVersionId === versionId
			) {
				if (!seeded.result.ok) throw new Error(seeded.result.error);
				loaded = seeded.result.data;
			} else {
				loaded = await getHomeroomDeliveryWorkspace(yearId, termId, {
					deliveryVersionId: versionId,
					signal: ticket.signal
				});
			}
			if (!current(owner, ticket.revision)) return;
			if (
				loaded.academicYearId !== yearId ||
				loaded.academicTermId !== termId ||
				loaded.deliveryVersionId !== versionId
			)
				throw new Error('ข้อมูลชื่อห้องไม่ตรงกับรุ่นที่เลือก กรุณาโหลดอีกครั้ง');
			workspace = loaded;
		} catch (failure) {
			if (!isAbortError(failure) && current(owner, ticket.revision)) {
				workspace = null;
				error = failure instanceof Error ? failure.message : 'โหลดชื่อห้องในรุ่นเปิดสอนไม่สำเร็จ';
			}
		} finally {
			if (current(owner, ticket.revision)) loading = false;
		}
	}
	$effect(() => {
		const owner = context,
			seeded = initialWorkspace,
			needed = hasAudience;
		untrack(() => {
			workspace = null;
			if (needed) void load(seeded);
			else {
				request.abort();
				loading = false;
				error = '';
			}
		});
		return () => {
			if (owner === context) request.abort();
		};
	});
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
</script>

<section
	class="overflow-hidden rounded-xl border bg-card"
	aria-label="ห้องที่เปิดสอนในรุ่นนี้"
	aria-busy={loading}
>
	<header class="flex flex-wrap items-center justify-between gap-2 border-b px-4 py-3">
		<h2 class="font-semibold">ห้องที่เปิดสอนในรุ่นนี้</h2>
		<span class="text-sm text-muted-foreground">{offering.homeroomIds.length} ห้อง</span>
		<p class="w-full text-xs text-muted-foreground">
			ขอบเขตห้องของรายการในรุ่นที่เลือก แยกจากกลุ่มเรียนและประวัติรายชื่อเดิม
		</p>
	</header>
	{#if loading}<div class="p-4"><PageSkeleton variant="cards" rows={1} /></div>
	{:else if error}<div class="space-y-2 p-4">
			<p role="alert" class="text-sm text-destructive">{error}</p>
			<Button size="sm" variant="outline" onclick={() => load()}>โหลดชื่อห้องอีกครั้ง</Button>
		</div>
	{:else if rooms.length === 0}<p class="p-4 text-sm text-muted-foreground">
			ยังไม่ระบุห้องที่เปิดสอนในรุ่นนี้
		</p>
	{:else}<ul class="max-h-72 divide-y overflow-y-auto">
			{#each rooms as room (room.id)}<li
					class="flex flex-wrap items-center justify-between gap-2 px-4 py-3"
				>
					<div class="min-w-0">
						<h3 class="text-sm font-medium">{room.name ?? 'ไม่พบชื่อห้องในข้อมูลภาคเรียนนี้'}</h3>
						<p class="text-xs text-muted-foreground">
							{room.groupCount
								? `มีกลุ่มเรียนสำหรับห้องนี้ ${room.groupCount} กลุ่ม`
								: offering.catalog.kind === 'activity' &&
									  offering.catalog.schedulingMode === 'synchronized'
									? 'เปิดแบบกลาง ยังไม่มีกลุ่มเรียนสำหรับห้องนี้'
									: 'ยังไม่มีกลุ่มเรียนสำหรับห้องนี้'}
						</p>
					</div>
					{#if room.name && canManage && version.status === 'draft' && changeSetId}<DeliveryDraftResourceRemoval
							{changeSetId}
							versionId={version.id}
							offeringId={offering.id}
							label={offering.name}
							homeroomId={room.id}
							homeroomName={room.name}
							{canManage}
							{onRemoved}
						/>{/if}
				</li>{/each}
		</ul>
		{#if rooms.some((room) => room.name === null)}<div class="space-y-2 border-t p-4">
				<p class="text-sm text-muted-foreground">
					ชื่อห้องบางส่วนยังไม่พร้อม จึงยังนำห้องเหล่านั้นออกไม่ได้
				</p>
				<Button size="sm" variant="outline" onclick={() => load()}>โหลดชื่อห้องอีกครั้ง</Button>
			</div>{/if}
	{/if}
</section>
