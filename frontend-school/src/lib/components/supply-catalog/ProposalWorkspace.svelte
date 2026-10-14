<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		listProposals,
		listCategories,
		getProposal,
		rejectProposal,
		type SupplyProposal,
		type SupplyProposalPage,
		type SupplyCategory,
		type SupplyItem
	} from '#lib/api/supply-catalog.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { money, message } from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import CatalogImage from './CatalogImage.svelte';
	import ItemEditor from './ItemEditor.svelte';
	import { toast } from 'svelte-sonner';
	let {
		identity,
		initialProposals,
		initialCategories,
		query,
		requestKey,
		canPropose,
		canApprove
	}: {
		identity: string;
		initialProposals: Promise<
			RouteLoadResult<{ ownerKey: string; page: SupplyProposalPage | null }>
		>;
		initialCategories: Promise<
			RouteLoadResult<{ ownerKey: string; categories: SupplyCategory[] | null }>
		>;
		query: { review: boolean; page: number; pageSize: number };
		requestKey: string;
		canPropose: boolean;
		canApprove: boolean;
	} = $props();
	const request = new LatestRequest(),
		categoryRequest = new LatestRequest(),
		detailRequest = new LatestRequest();
	let disposed = false,
		result = $state.raw<SupplyProposalPage | null>(null),
		categories = $state.raw<SupplyCategory[]>([]);
	let loading = $state(true),
		error = $state<string | null>(null),
		categoriesLoading = $state(true),
		categoriesError = $state<string | null>(null);
	let mode = $state<'detail' | 'propose' | 'approve' | null>(null),
		selected = $state.raw<SupplyProposal | null>(null),
		selectedId = $state<string | null>(null),
		detailLoading = $state(false),
		detailError = $state<string | null>(null);
	let rejecting = $state(false),
		reason = $state(''),
		rejectPending = $state(false),
		rejectError = $state<string | null>(null),
		editor = $state<{ discard: () => Promise<boolean> } | undefined>(undefined);
	function status(record: SupplyProposal) {
		return record.status === 'approved'
			? 'อนุมัติแล้ว'
			: record.status === 'rejected'
				? 'ไม่อนุมัติ'
				: 'รออนุมัติ';
	}
	$effect.pre(() => {
		const source = initialProposals,
			key = `${identity}|${requestKey}`;
		untrack(() => {
			const ticket = request.begin();
			result = null;
			loading = true;
			error = null;
			mode = null;
			detailRequest.abort();
			void source.then((loaded) => {
				if (!request.isCurrent(ticket.revision)) return;
				loading = false;
				if (!loaded.ok) error = loaded.error;
				else if (loaded.data.ownerKey === key) result = loaded.data.page;
			});
		});
		return () => request.abort();
	});
	$effect.pre(() => {
		const source = initialCategories,
			key = identity;
		untrack(() => {
			const ticket = categoryRequest.begin();
			categoriesLoading = true;
			categoriesError = null;
			void source.then((loaded) => {
				if (!categoryRequest.isCurrent(ticket.revision)) return;
				categoriesLoading = false;
				if (!loaded.ok) categoriesError = loaded.error;
				else if (loaded.data.ownerKey === key) categories = loaded.data.categories ?? [];
			});
		});
		return () => categoryRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		request.abort();
		categoryRequest.abort();
		detailRequest.abort();
	});
	async function load() {
		const ticket = request.begin();
		loading = true;
		error = null;
		try {
			const loaded = await listProposals(query, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) result = loaded;
		} catch (e) {
			if (request.isCurrent(ticket.revision)) error = message(e, 'โหลดข้อเสนอไม่สำเร็จ');
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	async function loadCategories() {
		const ticket = categoryRequest.begin();
		categoriesLoading = true;
		categoriesError = null;
		try {
			const loaded = await listCategories({ signal: ticket.signal });
			if (categoryRequest.isCurrent(ticket.revision)) categories = loaded;
		} catch (e) {
			if (categoryRequest.isCurrent(ticket.revision))
				categoriesError = message(e, 'โหลดหมวดหมู่ไม่สำเร็จ');
		} finally {
			if (categoryRequest.isCurrent(ticket.revision)) categoriesLoading = false;
		}
	}
	function navigate(next: number) {
		const url = new URL(page.url.href);
		url.searchParams.set('page', String(next));
		void goto(url.pathname + url.search, { reset: false });
	}
	async function detail(id: string) {
		mode = 'detail';
		selectedId = id;
		selected = null;
		detailLoading = true;
		detailError = null;
		const ticket = detailRequest.begin();
		try {
			const loaded = await getProposal(id, { signal: ticket.signal });
			if (detailRequest.isCurrent(ticket.revision)) selected = loaded;
		} catch (e) {
			if (detailRequest.isCurrent(ticket.revision))
				detailError = message(e, 'โหลดข้อเสนอไม่สำเร็จ');
		} finally {
			if (detailRequest.isCurrent(ticket.revision)) detailLoading = false;
		}
	}
	async function close() {
		if (rejectPending || rejecting) return;
		if (editor && !(await editor.discard())) return;
		mode = null;
		selected = null;
		detailRequest.abort();
	}
	function patch(record: SupplyProposal) {
		request.abort();
		loading = false;
		if (!result) return;
		const exists = result.items.some((item) => item.id === record.id);
		result = {
			...result,
			items:
				query.review && record.status !== 'pending'
					? result.items.filter((item) => item.id !== record.id)
					: exists
						? result.items.map((item) => (item.id === record.id ? record : item))
						: [record, ...result.items].slice(0, result.pageSize),
			total: result.total + (exists ? (query.review && record.status !== 'pending' ? -1 : 0) : 1)
		};
	}
	function saved(resource: SupplyItem | SupplyProposal) {
		if ('latestVersion' in resource && selected)
			patch({
				...selected,
				status: 'approved',
				imageFileId: null,
				sourceFileId: null,
				sourceUrl: null,
				itemId: resource.id
			});
		else if ('status' in resource) patch(resource);
		mode = null;
		selected = null;
		toast.success('บันทึกข้อเสนอแล้ว');
	}
	async function reject() {
		if (!selected || rejectPending) return;
		if (!reason.trim()) {
			rejectError = 'กรอกเหตุผลที่ไม่อนุมัติ';
			return;
		}
		rejectPending = true;
		rejectError = null;
		try {
			const saved = await rejectProposal(selected.id, {
				revision: selected.revision,
				reason: reason.trim()
			});
			if (!disposed) {
				patch(saved);
				rejecting = false;
				selected = saved;
				mode = null;
				toast.success('บันทึกผลพิจารณาแล้ว');
			}
		} catch (e) {
			if (!disposed) rejectError = message(e, 'บันทึกผลพิจารณาไม่สำเร็จ');
		} finally {
			if (!disposed) rejectPending = false;
		}
	}
</script>

<div class="space-y-5" data-testid="supply-proposals">
	<div class="flex flex-wrap justify-end gap-2">
		{#if canApprove && canPropose}<Button
				variant="outline"
				href={`${'/staff/supply-catalog/proposals'}?review=${!query.review}`}
				>{query.review ? 'ข้อเสนอของฉัน' : 'ตรวจข้อเสนอรออนุมัติ'}</Button
			>{/if}{#if canPropose}<Button
				disabled={categoriesLoading || !!categoriesError || categories.length === 0}
				onclick={() => {
					selected = null;
					mode = 'propose';
				}}>เสนอพัสดุใหม่</Button
			>{/if}
	</div>
	{#if categoriesError}<PageState
			variant="error"
			title="โหลดหมวดหมู่ไม่สำเร็จ"
			description={categoriesError}
			actionLabel="ลองโหลดหมวดหมู่"
			onaction={loadCategories}
		/>{/if}
	{#if loading && !result}<PageSkeleton variant="cards" rows={3} />{/if}
	{#if error}<PageState
			variant="error"
			title="โหลดข้อเสนอไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={load}
		/>{/if}
	{#if result}<section class="space-y-4" aria-busy={loading}>
			{#if result.items.length === 0}<PageState
					title={query.review ? 'ไม่มีข้อเสนอรออนุมัติ' : 'ยังไม่มีข้อเสนอของคุณ'}
					description={query.review
						? 'ข้อเสนอที่พิจารณาแล้วจะอยู่ในประวัติของผู้เสนอ'
						: 'เสนอพัสดุพร้อมราคาและร้านค้าเพื่อให้ผู้รับผิดชอบตรวจสอบ'}
				/>{:else}<div class="grid gap-4 lg:grid-cols-2">
					{#each result.items as proposal (proposal.id)}<Card.Root class="gap-0 py-0"
							><Card.Content class="space-y-3 p-4"
								><div class="flex items-start gap-2">
									<h3 class="min-w-0 flex-1 break-words font-semibold">{proposal.name}</h3>
									<Badge variant={proposal.status === 'rejected' ? 'destructive' : 'secondary'}
										>{status(proposal)}</Badge
									>
								</div>
								<p class="text-sm text-muted-foreground">
									{categories.find((category) => category.id === proposal.categoryId)?.name ??
										'หมวดหมู่พัสดุ'} · {proposal.unit}
								</p>
								<p class="text-lg font-semibold">
									{money(proposal.price)} ฿
									<span class="text-sm font-normal text-muted-foreground">{proposal.shop}</span>
								</p>
								<div class="flex items-center justify-between gap-2">
									<p class="text-xs text-muted-foreground">
										{new Date(proposal.createdAt).toLocaleDateString('th-TH')}
									</p>
									<Button size="sm" variant="outline" onclick={() => detail(proposal.id)}
										>{query.review && proposal.status === 'pending'
											? 'ตรวจข้อเสนอ'
											: 'ดูข้อเสนอ'}</Button
									>
								</div></Card.Content
							></Card.Root
						>{/each}
				</div>{/if}
			<div class="flex items-center justify-between gap-3">
				<Button
					size="sm"
					variant="outline"
					disabled={loading || result.page === 1}
					onclick={() => navigate(result!.page - 1)}>ก่อนหน้า</Button
				><span class="text-sm text-muted-foreground"
					>หน้า {result.page} / {Math.max(1, Math.ceil(result.total / result.pageSize))}</span
				><Button
					size="sm"
					variant="outline"
					disabled={loading || result.page * result.pageSize >= result.total}
					onclick={() => navigate(result!.page + 1)}>ถัดไป</Button
				>
			</div>
		</section>{/if}
</div>
<Dialog.Root
	open={mode !== null}
	onOpenChange={(open) => {
		if (!open) void close();
	}}
	><Dialog.Content class="max-h-[85dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title
				>{mode === 'propose'
					? 'เสนอพัสดุเข้าคลัง'
					: mode === 'approve'
						? 'ตรวจและอนุมัติข้อเสนอ'
						: (selected?.name ?? 'รายละเอียดข้อเสนอ')}</Dialog.Title
			><Dialog.Description
				>{mode === 'approve'
					? 'แก้ไขข้อมูลทุกช่องก่อนอนุมัติ แหล่งที่มาจะถูกลบหลังรับเข้าคลัง'
					: 'ติดตามข้อมูลและผลการพิจารณาข้อเสนอ'}</Dialog.Description
			></Dialog.Header
		>
		{#if mode === 'propose'}<ItemEditor
				bind:this={editor}
				mode="propose"
				{categories}
				onsaved={saved}
				oncancel={() => (mode = null)}
			/>
		{:else if detailLoading}<PageSkeleton variant="detail" rows={3} />
		{:else if detailError}<PageState
				variant="error"
				title="โหลดข้อเสนอไม่สำเร็จ"
				description={detailError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => selectedId && detail(selectedId)}
			/>
		{:else if selected}
			{#if selected.status === 'pending' && (selected.sourceUrl || selected.sourceFileId)}<section
					class="space-y-2 rounded-xl border p-3"
				>
					<h3 class="text-sm font-semibold">แหล่งที่มาชั่วคราว</h3>
					{#if selected.sourceUrl}<a
							href={selected.sourceUrl}
							target="_blank"
							rel="noopener noreferrer"
							class="break-all text-sm text-primary underline">เปิดลิงก์แหล่งที่มา</a
						>{/if}{#if selected.sourceFileId}<CatalogImage
							fileId={selected.sourceFileId}
							resourceId={selected.id}
							alt="รูปแหล่งที่มา"
							class="h-40 rounded-lg"
						/>{/if}
				</section>{/if}
			{#if mode === 'approve' && canApprove && selected.status === 'pending'}<ItemEditor
					bind:this={editor}
					mode="approve"
					proposal={selected}
					{categories}
					onsaved={saved}
					oncancel={() => (mode = 'detail')}
				/>
			{:else}<div class="space-y-4">
					{#if selected.imageFileId}<CatalogImage
							fileId={selected.imageFileId}
							resourceId={selected.id}
							alt={selected.name}
							class="h-44 rounded-xl border"
						/>{/if}<Badge variant="secondary">{status(selected)}</Badge>
					<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
						<dt class="text-muted-foreground">หมวดหมู่</dt>
						<dd class="break-words">
							{categories.find((category) => category.id === selected?.categoryId)?.name ?? '—'}
						</dd>
						<dt class="text-muted-foreground">หน่วยนับ</dt>
						<dd>{selected.unit}</dd>
						<dt class="text-muted-foreground">ราคา</dt>
						<dd>{money(selected.price)} ฿</dd>
						<dt class="text-muted-foreground">ร้านค้า</dt>
						<dd class="break-words">{selected.shop}</dd>
					</dl>
					{#if selected.description}<p class="whitespace-pre-wrap break-words text-sm">
							{selected.description}
						</p>{/if}{#if selected.rejectionReason}<p
							class="rounded-xl border border-destructive/20 p-3 text-sm"
						>
							เหตุผล: {selected.rejectionReason}
						</p>{/if}{#if selected.status === 'approved'}<p class="text-sm text-muted-foreground">
							รับเข้าคลังแล้ว แหล่งที่มาชั่วคราวถูกลบ
						</p>
						{#if selected.itemId}<Button variant="outline" href={'/staff/supply-catalog'}
								>เปิดคลังพัสดุ</Button
							>{/if}{/if}{#if canApprove && query.review && selected.status === 'pending'}<div
							class="flex justify-end gap-2"
						>
							<Button
								variant="destructive"
								onclick={() => {
									reason = '';
									rejectError = null;
									rejecting = true;
								}}>ไม่อนุมัติ</Button
							><Button
								disabled={categoriesLoading || !!categoriesError}
								onclick={() => (mode = 'approve')}>แก้ไขและอนุมัติ</Button
							>
						</div>{/if}
				</div>{/if}
		{/if}
	</Dialog.Content></Dialog.Root
>
<AlertDialog.Root
	open={rejecting}
	onOpenChange={(open) => {
		if (!open && !rejectPending) rejecting = false;
	}}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ไม่อนุมัติข้อเสนอ?</AlertDialog.Title><AlertDialog.Description
				>บันทึกเหตุผลให้ผู้เสนอตรวจสอบและส่งข้อมูลใหม่ได้</AlertDialog.Description
			></AlertDialog.Header
		>
		<div class="space-y-2">
			<Label for="proposal-reject-reason">เหตุผล *</Label><Textarea
				id="proposal-reject-reason"
				bind:value={reason}
				maxlength={2000}
				disabled={rejectPending}
			/>{#if rejectError}<p role="alert" class="text-sm text-destructive">{rejectError}</p>{/if}
		</div>
		<AlertDialog.Footer
			><Button variant="outline" disabled={rejectPending} onclick={() => (rejecting = false)}
				>ยกเลิก</Button
			><LoadingButton variant="destructive" loading={rejectPending} onclick={reject}
				>ยืนยันไม่อนุมัติ</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
