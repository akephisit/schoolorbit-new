<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		listItems,
		listCategories,
		getItem,
		listVersions,
		setFavorite,
		type SupplyItem,
		type SupplyVersion,
		type SupplyItemDetail,
		type SupplyItemPage,
		type SupplyCategory,
		type SupplyProposal
	} from '#lib/api/supply-catalog.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { money, message, itemFilters } from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Heart, Plus, Layers, Settings2 } from '@lucide/svelte';
	import CatalogImage from './CatalogImage.svelte';
	import ItemEditor from './ItemEditor.svelte';
	import VersionEditor from './VersionEditor.svelte';
	import CategoryManager from './CategoryManager.svelte';
	import BundleManager from './BundleManager.svelte';
	import { toast } from 'svelte-sonner';
	type Query = ReturnType<typeof itemFilters>;
	let {
		identity,
		initialItems,
		initialCategories,
		query,
		requestKey,
		canManage,
		canPropose,
		canApprove,
		userId
	}: {
		identity: string;
		initialItems: Promise<RouteLoadResult<{ ownerKey: string; page: SupplyItemPage | null }>>;
		initialCategories: Promise<
			RouteLoadResult<{ ownerKey: string; categories: SupplyCategory[] | null }>
		>;
		query: Query;
		requestKey: string;
		canManage: boolean;
		canPropose: boolean;
		canApprove: boolean;
		userId: string;
	} = $props();
	const itemRequest = new LatestRequest(),
		categoryRequest = new LatestRequest(),
		detailRequest = new LatestRequest(), historyRequest = new LatestRequest();
	let disposed = false;
	let result = $state.raw<SupplyItemPage | null>(null),
		categories = $state.raw<SupplyCategory[]>([]),
		itemsLoading = $state(true),
		categoriesLoading = $state(true);
	let itemsError = $state<string | null>(null),
		categoriesError = $state<string | null>(null),
		actionError = $state<string | null>(null);
	let search = $state(untrack(() => query.search)),
		categoryId = $state(untrack(() => query.categoryId)),
		favoritesOnly = $state(untrack(() => query.favoritesOnly));
	let favoritePending = $state<string[]>([]);
	let mode = $state<
		| 'detail'
		| 'create'
		| 'edit'
		| 'propose'
		| 'append'
		| 'version'
		| 'categories'
		| 'bundles'
		| null
	>(null);
	let selected = $state.raw<SupplyItemDetail | null>(null),
		selectedId = $state<string | null>(null),
		detailLoading = $state(false),
		detailError = $state<string | null>(null);
	let history = $state.raw<SupplyVersion[]>([]), historyPage = $state(1), historyLoading = $state(false), historyError = $state<string | null>(null), versionTarget = $state.raw<SupplyVersion | null>(null);
	let editor = $state<{ discard: () => Promise<boolean> } | undefined>(undefined);
	let actionOwner = $state<{ discard: () => Promise<boolean> } | undefined>(undefined);
	$effect.pre(() => {
		const source = initialItems,
			key = `${identity}|${requestKey}`,
			nextQuery = query;
		untrack(() => {
			const ticket = itemRequest.begin();
			result = null;
			itemsLoading = true;
			itemsError = null;
			search = nextQuery.search;
			categoryId = nextQuery.categoryId;
			favoritesOnly = nextQuery.favoritesOnly;
			void source.then((loaded) => {
				if (!itemRequest.isCurrent(ticket.revision)) return;
				itemsLoading = false;
				if (!loaded.ok) itemsError = loaded.error;
				else if (loaded.data.ownerKey === key) result = loaded.data.page;
			});
		});
		return () => itemRequest.abort();
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
		itemRequest.abort();
		categoryRequest.abort();
		detailRequest.abort();
		historyRequest.abort();
	});
	async function retryItems() {
		const ticket = itemRequest.begin();
		itemsLoading = true;
		itemsError = null;
		try {
			const data = await listItems(
				{ ...query, categoryId: query.categoryId || undefined },
				{ signal: ticket.signal }
			);
			if (itemRequest.isCurrent(ticket.revision)) result = data;
		} catch (e) {
			if (itemRequest.isCurrent(ticket.revision)) itemsError = message(e, 'โหลดคลังไม่สำเร็จ');
		} finally {
			if (itemRequest.isCurrent(ticket.revision)) itemsLoading = false;
		}
	}
	async function retryCategories() {
		const ticket = categoryRequest.begin();
		categoriesLoading = true;
		categoriesError = null;
		try {
			const data = await listCategories({ signal: ticket.signal });
			if (categoryRequest.isCurrent(ticket.revision)) categories = data;
		} catch (e) {
			if (categoryRequest.isCurrent(ticket.revision))
				categoriesError = message(e, 'โหลดหมวดหมู่ไม่สำเร็จ');
		} finally {
			if (categoryRequest.isCurrent(ticket.revision)) categoriesLoading = false;
		}
	}
	function navigate(nextPage = 1) {
		const url = new URL(page.url.href);
		url.searchParams.delete('search');
		url.searchParams.delete('categoryId');
		url.searchParams.delete('favoritesOnly');
		url.searchParams.set('page', String(nextPage));
		if (search.trim()) url.searchParams.set('search', search.trim());
		if (categoryId) url.searchParams.set('categoryId', categoryId);
		if (favoritesOnly) url.searchParams.set('favoritesOnly', 'true');
		void goto(url.pathname + url.search, { reset: false });
	}
	function patchItem(item: SupplyItem, created = false) {
		itemRequest.abort();
		itemsLoading = false;
		if (!result) return;
		const exists = result.items.some((row) => row.id === item.id);
		const visible =
			(!query.categoryId || item.categoryId === query.categoryId) &&
			(!query.favoritesOnly || item.favorite);
		const rows = result.items
			.map((row) => (row.id === item.id ? item : row))
			.filter((row) => row.id !== item.id || visible);
		if (created && visible && !query.search && query.page === 1 && !exists) rows.unshift(item);
		result = {
			...result,
			items: rows.slice(0, result.pageSize),
			total: result.total + (created ? 1 : exists && !visible ? -1 : 0)
		};
	}
	async function favorite(item: SupplyItem) {
		if (favoritePending.includes(item.id)) return;
		favoritePending = [...favoritePending, item.id];
		actionError = null;
		try {
			const saved = await setFavorite(item.id, !item.favorite);
			if (!disposed) {
				patchItem(saved);
				if (selected?.id === saved.id) selected = { ...selected, ...saved };
			}
		} catch (e) {
			if (!disposed) actionError = message(e, 'บันทึกรายการโปรดไม่สำเร็จ');
		} finally {
			if (!disposed) favoritePending = favoritePending.filter((id) => id !== item.id);
		}
	}
	async function detail(id: string) {
		selectedId = id;
		mode = 'detail';
		selected = null;
		history = []; historyPage = 1; historyError = null; historyRequest.abort();
		detailLoading = true;
		detailError = null;
		const ticket = detailRequest.begin();
		try {
			const loaded = await getItem(id, { signal: ticket.signal });
			if (detailRequest.isCurrent(ticket.revision)) { selected = loaded; history = loaded.versions; }
		} catch (e) {
			if (detailRequest.isCurrent(ticket.revision))
				detailError = message(e, 'โหลดรายละเอียดไม่สำเร็จ');
		} finally {
			if (detailRequest.isCurrent(ticket.revision)) detailLoading = false;
		}
	}
	async function close() {
		if (editor && !(await editor.discard())) return;
		if (actionOwner && !(await actionOwner.discard())) return;
		mode = null;
		selected = null;
		selectedId = null;
		detailRequest.abort();
		historyRequest.abort();
	}
	async function loadHistory(next: number) { if (!selected) return; const itemId = selected.id; const ticket = historyRequest.begin(); historyLoading = true; historyError = null; try { const loaded = await listVersions(itemId, { page: next, pageSize: 100 }, { signal: ticket.signal }); if (historyRequest.isCurrent(ticket.revision)) { history = loaded.items; historyPage = loaded.page; } } catch (e) { if (historyRequest.isCurrent(ticket.revision)) historyError = message(e, 'โหลดประวัติราคาไม่สำเร็จ'); } finally { if (historyRequest.isCurrent(ticket.revision)) historyLoading = false; } }
	function editVersion(version: SupplyVersion) { versionTarget = version; mode = 'version'; }
	function saved(resource: SupplyItem | SupplyProposal) {
		if ('latestVersion' in resource) patchItem(resource, mode === 'create');
		mode = null;
		toast.success('บันทึกแล้ว');
	}
	function versionSaved(item: SupplyItemDetail) {
		selected = item;
		patchItem(item);
		if (historyPage === 1) history = item.versions; else void loadHistory(historyPage);
		mode = 'detail';
		toast.success('บันทึกราคาแล้ว');
	}
	function patchCategory(category: SupplyCategory, deleted = false) {
		categories = deleted
			? categories.filter((row) => row.id !== category.id)
			: [...categories.filter((row) => row.id !== category.id), category].toSorted((a, b) =>
					a.name.localeCompare(b.name, 'th')
				);
		if (result && !deleted)
			result = {
				...result,
				items: result.items.map((row) =>
					row.categoryId === category.id ? { ...row, categoryName: category.name } : row
				)
			};
	}
</script>

<div class="space-y-5" data-testid="supply-catalog">
	<div class="flex flex-wrap items-center justify-end gap-2">
		<Button variant="outline" onclick={() => (mode = 'bundles')}
			><Layers class="size-4" />ชุดพัสดุ</Button
		>{#if canPropose || canApprove}<Button
				variant="outline"
				href={`${'/staff/supply-catalog/proposals'}${canApprove && !canPropose ? '?review=true' : ''}`}
				>ข้อเสนอพัสดุ</Button
			>{/if}{#if canManage}<Button
				variant="outline"
				disabled={categoriesLoading}
				onclick={() => (mode = 'categories')}
				aria-label="จัดการหมวดหมู่"
				><Settings2 class="size-4" /><span class="hidden sm:inline">หมวดหมู่</span></Button
			><Button
				disabled={categoriesLoading || !!categoriesError || categories.length === 0}
				onclick={() => {
					selected = null;
					mode = 'create';
				}}><Plus class="size-4" />เพิ่มพัสดุ</Button
			>{:else if canPropose}<Button
				disabled={categoriesLoading || !!categoriesError || categories.length === 0}
				onclick={() => (mode = 'propose')}><Plus class="size-4" />เสนอพัสดุ</Button
			>{/if}
	</div>
	<form
		class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-3 sm:p-4"
		onsubmit={(e) => {
			e.preventDefault();
			navigate();
		}}
	>
		<div class="min-w-0 flex-1 basis-48 space-y-1">
			<Label for="catalog-search">ค้นหาพัสดุ</Label><Input
				id="catalog-search"
				bind:value={search}
				placeholder="ชื่อพัสดุหรือรายละเอียด"
			/>
		</div>
		<div class="w-full space-y-1 sm:w-48">
			<Label for="catalog-category">หมวดหมู่</Label><Select.Root
				type="single"
				bind:value={categoryId}
				disabled={categoriesLoading || !!categoriesError}
				><Select.Trigger id="catalog-category" class="w-full"
					>{categories.find((row) => row.id === categoryId)?.name ?? 'ทุกหมวดหมู่'}</Select.Trigger
				><Select.Content
					><Select.Item value="">ทุกหมวดหมู่</Select.Item
					>{#each categories as category (category.id)}<Select.Item value={category.id}
							>{category.name}</Select.Item
						>{/each}</Select.Content
				></Select.Root
			>
		</div>
		<div class="flex h-9 items-center gap-2">
			<Checkbox id="catalog-favorites" bind:checked={favoritesOnly} /><Label for="catalog-favorites"
				>รายการโปรด</Label
			>
		</div>
		<Button type="submit">ค้นหา</Button>
	</form>
	{#if categoriesError}<PageState
			variant="error"
			title="โหลดหมวดหมู่ไม่สำเร็จ"
			description={categoriesError}
			actionLabel="ลองโหลดหมวดหมู่"
			onaction={retryCategories}
		/>{/if}
	{#if actionError}<p role="alert" class="text-sm text-destructive">{actionError}</p>{/if}
	{#if itemsLoading && !result}<PageSkeleton variant="cards" rows={6} />{/if}
	{#if itemsError}<PageState
			variant="error"
			title="โหลดคลังพัสดุไม่สำเร็จ"
			description={itemsError}
			actionLabel="ลองอีกครั้ง"
			onaction={retryItems}
		/>{/if}
	{#if result}<section class="space-y-4" aria-busy={itemsLoading}>
			<div class="flex items-center justify-between">
				<p class="text-sm text-muted-foreground">{result.total.toLocaleString('th-TH')} รายการ</p>
				{#if itemsLoading}<span role="status" class="text-xs text-muted-foreground"
						>กำลังอัปเดต</span
					>{/if}
			</div>
			{#if result.items.length === 0}<PageState
					title={query.search || query.categoryId || query.favoritesOnly
						? 'ไม่พบพัสดุที่ตรงกับตัวกรอง'
						: 'คลังพัสดุยังว่าง'}
					description="ลองเปลี่ยนคำค้นหรือหมวดหมู่"
				/>{:else}<div
					class="grid grid-cols-1 gap-4 min-[480px]:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4"
				>
					{#each result.items as item (item.id)}<Card.Root class="gap-0 overflow-hidden py-0"
							><CatalogImage
								fileId={item.imageFileId}
								resourceId={item.id}
								alt={item.name}
								class="h-40 border-b sm:h-44"
							/><Card.Content class="space-y-3 p-4"
								><div class="flex items-start gap-2">
									<Button
										variant="ghost"
										class="h-auto min-w-0 flex-1 justify-start p-0 text-left text-base font-semibold whitespace-normal hover:bg-transparent"
										onclick={() => detail(item.id)}>{item.name}</Button
									><Button
										variant="ghost"
										size="icon"
										class="shrink-0"
										aria-label={item.favorite
											? `นำ ${item.name} ออกจากรายการโปรด`
											: `เพิ่ม ${item.name} เป็นรายการโปรด`}
										aria-pressed={item.favorite}
										disabled={favoritePending.includes(item.id)}
										onclick={() => favorite(item)}
										><Heart
											class={item.favorite ? 'size-4 fill-primary text-primary' : 'size-4'}
										/></Button
									>
								</div>
								<Badge variant="secondary">{item.categoryName}</Badge>
								<div>
									<p class="text-lg font-semibold tabular-nums">
										{money(item.latestVersion.price)}
										<span class="text-sm font-normal text-muted-foreground">฿ / {item.unit}</span>
									</p>
									<p class="break-words text-sm text-muted-foreground">{item.latestVersion.shop}</p>
								</div>
								<Button variant="outline" size="sm" class="w-full" onclick={() => detail(item.id)}
									>ดูรายละเอียด</Button
								></Card.Content
							></Card.Root
						>{/each}
				</div>{/if}
			<div class="flex items-center justify-between gap-3">
				<Button
					variant="outline"
					size="sm"
					disabled={itemsLoading || result.page === 1}
					onclick={() => navigate(result!.page - 1)}>ก่อนหน้า</Button
				><span class="text-sm text-muted-foreground"
					>หน้า {result.page} / {Math.max(1, Math.ceil(result.total / result.pageSize))}</span
				><Button
					variant="outline"
					size="sm"
					disabled={itemsLoading || result.page * result.pageSize >= result.total}
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
				>{mode === 'create'
					? 'เพิ่มพัสดุ'
					: mode === 'edit'
						? 'แก้ไขข้อมูลและรูปสินค้า'
						: mode === 'propose'
							? 'เสนอพัสดุเข้าคลัง'
							: mode === 'append'
								? 'เพิ่มรุ่นราคาใหม่'
								: mode === 'version'
									? 'แก้ไขรุ่นราคาปัจจุบัน'
									: mode === 'categories'
										? 'จัดการหมวดหมู่'
										: mode === 'bundles'
											? 'ชุดพัสดุ'
											: (selected?.name ?? 'รายละเอียดพัสดุ')}</Dialog.Title
			><Dialog.Description
				>{mode === 'detail'
					? 'ดูข้อมูลปัจจุบันและประวัติราคา'
					: mode === 'bundles'
						? 'เก็บรายการที่ใช้บ่อยพร้อมจำนวน'
						: 'กรอกข้อมูลให้ครบ แล้วบันทึกการเปลี่ยนแปลง'}</Dialog.Description
			></Dialog.Header
		>
		{#if mode === 'detail'}{#if detailLoading}<PageSkeleton
					variant="detail"
					rows={3}
				/>{:else if detailError}<PageState
					variant="error"
					title="โหลดรายละเอียดไม่สำเร็จ"
					description={detailError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => selectedId && detail(selectedId)}
				/>{:else if selected}<div class="space-y-4">
					<CatalogImage
						fileId={selected.imageFileId}
						resourceId={selected.id}
						alt={selected.name}
						class="h-48 rounded-xl border"
					/>
					<div class="flex flex-wrap gap-2">
						<Badge variant="secondary">{selected.categoryName}</Badge><Badge variant="outline"
							>{selected.unit}</Badge
						>
					</div>
					{#if selected.description}<p class="whitespace-pre-wrap break-words text-sm">
							{selected.description}
						</p>{/if}
					<section class="rounded-xl border border-primary/20 bg-primary/5 p-4">
						<p class="text-sm text-muted-foreground">
							ราคาล่าสุด · รุ่น {selected.latestVersion.version}
						</p>
						<p class="text-2xl font-semibold">
							{money(selected.latestVersion.price)} ฿ / {selected.unit}
						</p>
						<p class="break-words text-sm">{selected.latestVersion.shop}</p>
					</section>
					{#if canManage}<div class="flex flex-wrap gap-2">
							<Button variant="outline" onclick={() => (mode = 'edit')}>แก้ไขข้อมูล / รูป</Button
							><Button variant="outline" onclick={() => selected && editVersion(selected.latestVersion)}
								>แก้ไขรุ่นปัจจุบัน</Button
							><Button onclick={() => (mode = 'append')}>เพิ่มรุ่นราคา</Button>
						</div>{/if}
					<section class="space-y-2">
						<h3 class="font-semibold">ประวัติราคา ({selected.versionsTotal} รุ่น)</h3>
						{#if selected.versionsTotal > selected.versions.length}<p
								class="text-xs text-muted-foreground"
							>
								แสดงครั้งละ 100 รุ่น เรียงจากใหม่ไปเก่า
							</p>{/if}
						<ul class="divide-y rounded-xl border">
							{#each history as version (version.id)}<li
									class="flex justify-between gap-3 p-3 text-sm"
								>
									<span class="min-w-0 break-words"
										>รุ่น {version.version}<span class="block text-xs text-muted-foreground"
											>{version.shop} · {new Date(version.createdAt).toLocaleDateString(
												'th-TH'
											)}</span
										></span
									><div class="flex shrink-0 items-center gap-2"><span class="tabular-nums">{money(version.price)} ฿</span>{#if canManage}<Button variant="ghost" size="sm" onclick={() => editVersion(version)} aria-label={`แก้ไขรุ่น ${version.version}`}>แก้ไข</Button>{/if}</div>
								</li>{/each}
						</ul>
					</section>
				</div>{/if}
		{:else if mode === 'create' || mode === 'edit' || mode === 'propose'}{#key mode}<ItemEditor
					bind:this={editor}
					{mode}
					item={mode === 'edit' ? selected : null}
					{categories}
					onsaved={saved}
					oncancel={() => (mode = null)}
				/>{/key}
		{:else if (mode === 'append' || mode === 'version') && selected}{#key mode}<VersionEditor
					bind:this={actionOwner}
					item={selected}
					version={mode === 'version' ? versionTarget : null}
					append={mode === 'append'}
					onsaved={versionSaved}
					oncancel={() => (mode = 'detail')}
				/>{/key}
		{:else if mode === 'categories' && canManage}<CategoryManager
				bind:this={actionOwner}
				{categories}
				onpatch={patchCategory}
			/>
		{:else if mode === 'bundles'}<BundleManager bind:this={actionOwner} {userId} {canManage} />{/if}
	</Dialog.Content></Dialog.Root
>
