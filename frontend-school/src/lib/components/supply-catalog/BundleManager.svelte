<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		listBundles,
		saveBundle,
		deleteBundle,
		type SupplyBundle,
		type SupplyBundlePage,
		type SupplyItem
	} from '#lib/api/supply-catalog.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import {
		message,
		money,
		subtotal,
		sumPrices,
		validateSupplyText,
		trimSupplyText
	} from '#lib/supply-catalog/drafts.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { AlertCircle } from '@lucide/svelte';
	import FieldError from './FieldError.svelte';
	import ItemPicker from './ItemPicker.svelte';
	let { userId, canManage }: { userId: string; canManage: boolean } = $props();
	const request = new LatestRequest();
	let disposed = false;
	let result = $state.raw<SupplyBundlePage | null>(null),
		loading = $state(true),
		error = $state<string | null>(null),
		pending = $state(false);
	let selected = $state.raw<SupplyBundle | null>(null),
		editing = $state(false),
		name = $state(''),
		shared = $state(false);
	let entries = $state<{ item: SupplyItem; quantity: number }[]>([]),
		deleting = $state.raw<SupplyBundle | null>(null);
	let submitted = $state(false);
	const nameError = $derived(submitted ? validateSupplyText(name, 'ชื่อชุดพัสดุ', 200) : null);
	const entriesError = $derived(
		submitted && entries.length === 0 ? 'เพิ่มพัสดุอย่างน้อยหนึ่งรายการ' : null
	);
	function quantityError(quantity: number): string | null {
		return Number.isSafeInteger(quantity) && quantity >= 1 && quantity <= 1000000
			? null
			: 'จำนวนต้องเป็นจำนวนเต็มตั้งแต่ 1 ถึง 1,000,000';
	}
	const total = $derived(
		sumPrices(entries.map((entry) => subtotal(entry.item.latestVersion.price, entry.quantity)))
	);
	function writable(bundle: SupplyBundle) {
		return bundle.shared ? canManage : bundle.ownerUserId === userId;
	}
	async function load(page = 1) {
		const ticket = request.begin();
		loading = true;
		error = null;
		try {
			const data = await listBundles({ page, pageSize: 10 }, { signal: ticket.signal });
			if (request.isCurrent(ticket.revision)) result = data;
		} catch (e) {
			if (request.isCurrent(ticket.revision)) error = message(e, 'โหลดชุดพัสดุไม่สำเร็จ');
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	$effect.pre(() => {
		void load();
		return () => request.abort();
	});
	export async function discard() {
		return !pending;
	}
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	function edit(bundle: SupplyBundle | null) {
		selected = bundle;
		name = bundle?.name ?? '';
		shared = bundle?.shared ?? false;
		entries =
			bundle?.entries.map((entry) => ({ item: entry.item, quantity: entry.quantity })) ?? [];
		editing = true;
		submitted = false;
		error = null;
	}
	function add(item: SupplyItem) {
		const found = entries.find((entry) => entry.item.id === item.id);
		if (found) found.quantity = Math.min(1000000, found.quantity + 1);
		else if (entries.length < 100) entries = [...entries, { item, quantity: 1 }];
		else error = 'ชุดพัสดุมีได้ไม่เกิน 100 รายการ';
	}
	async function save() {
		if (pending || disposed) return;
		submitted = true;
		error = null;
		if (nameError || entriesError || entries.some((entry) => quantityError(entry.quantity))) return;
		pending = true;
		error = null;
		try {
			const saved = await saveBundle(selected?.id ?? null, {
				name: trimSupplyText(name),
				shared,
				revision: selected?.revision ?? null,
				entries: entries.map((entry) => ({ itemId: entry.item.id, quantity: entry.quantity }))
			});
			if (!disposed) {
				request.abort();
				loading = false;
				if (result) {
					const exists = result.items.some((entry) => entry.id === saved.id);
					result = {
						...result,
						items: exists
							? result.items.map((entry) => (entry.id === saved.id ? saved : entry))
							: [saved, ...result.items].slice(0, result.pageSize),
						total: result.total + (exists ? 0 : 1)
					};
				}
				editing = false;
			}
		} catch (e) {
			if (!disposed) error = message(e, 'บันทึกชุดพัสดุไม่สำเร็จ');
		} finally {
			if (!disposed) pending = false;
		}
	}
	async function remove() {
		if (pending || !deleting) return;
		const target = deleting;
		pending = true;
		error = null;
		try {
			await deleteBundle(target.id, target.revision);
			if (!disposed && result) {
				request.abort();
				loading = false;
				result = {
					...result,
					items: result.items.filter((entry) => entry.id !== target.id),
					total: result.total - 1
				};
				deleting = null;
			}
		} catch (e) {
			if (!disposed) {
				error = message(e, 'ลบชุดพัสดุไม่สำเร็จ');
				deleting = null;
			}
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<div class="space-y-4" data-testid="supply-bundles">
	<p class="text-sm text-muted-foreground">ชุดเก็บพัสดุและจำนวน ราคาคำนวณจากรุ่นล่าสุดเสมอ</p>
	{#if error}<Alert.Root variant="destructive"
			><AlertCircle aria-hidden="true" /><Alert.Title>จัดการชุดพัสดุไม่สำเร็จ</Alert.Title
			><Alert.Description>{error}</Alert.Description></Alert.Root
		>{/if}
	{#if editing}<form
			class="space-y-5"
			novalidate
			onsubmit={(e) => {
				e.preventDefault();
				void save();
			}}
		>
			<section class="space-y-3" aria-labelledby="bundle-identity-heading">
				<h3 id="bundle-identity-heading" class="font-semibold">
					{selected ? 'แก้ไขชุดพัสดุ' : 'สร้างชุดพัสดุ'}
				</h3>
				<div class="space-y-1">
					<Label for="bundle-name">ชื่อชุดพัสดุ *</Label><Input
						id="bundle-name"
						bind:value={name}
						required
						disabled={pending}
						aria-invalid={!!nameError}
						aria-describedby={nameError ? 'bundle-name-error' : undefined}
					/><FieldError id="bundle-name-error" message={nameError} />
				</div>
				{#if canManage}<div class="flex items-center gap-2">
						<Checkbox id="bundle-shared" bind:checked={shared} disabled={pending} /><Label
							for="bundle-shared">ใช้ร่วมกันทั้งโรงเรียน</Label
						>
					</div>{/if}
			</section>
			<section class="space-y-3" aria-labelledby="bundle-items-heading">
				<h3 id="bundle-items-heading" class="font-semibold">รายการในชุด</h3>
				<ItemPicker onselect={add} disabled={pending} />
				<FieldError id="bundle-entries-error" message={entriesError} />
				<div class="space-y-3" aria-describedby={entriesError ? 'bundle-entries-error' : undefined}>
					{#each entries as entry (entry.item.id)}<Card.Root class="gap-0 py-0"
							><Card.Content class="p-3">
								<div class="flex items-start gap-2">
									<div class="min-w-0 flex-1">
										<p class="break-words text-sm font-medium">{entry.item.name}</p>
										<p class="text-xs text-muted-foreground">
											{money(entry.item.latestVersion.price)} ฿ / {entry.item.unit} · รุ่น {entry
												.item.latestVersion.version}
										</p>
									</div>
									<Button
										type="button"
										variant="ghost"
										size="sm"
										disabled={pending}
										onclick={() =>
											(entries = entries.filter((value) => value.item.id !== entry.item.id))}
										>นำออก</Button
									>
								</div>
								<div class="mt-2 flex items-center gap-3">
									<Label for={`quantity-${entry.item.id}`}>จำนวน</Label><Input
										id={`quantity-${entry.item.id}`}
										aria-label={`จำนวน ${entry.item.name}`}
										type="number"
										min={1}
										max={1000000}
										step={1}
										bind:value={entry.quantity}
										class="w-24"
										disabled={pending}
										aria-invalid={submitted && !!quantityError(entry.quantity)}
										aria-describedby={submitted && quantityError(entry.quantity)
											? `quantity-error-${entry.item.id}`
											: undefined}
									/><span class="ml-auto text-sm tabular-nums"
										>{money(subtotal(entry.item.latestVersion.price, entry.quantity))} ฿</span
									>
								</div>
								{#if submitted}<FieldError
										id={`quantity-error-${entry.item.id}`}
										message={quantityError(entry.quantity)}
									/>{/if}
							</Card.Content></Card.Root
						>{/each}
				</div>
			</section>
			<p class="text-right font-semibold">รวม {money(total)} ฿</p>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="outline" disabled={pending} onclick={() => (editing = false)}
					>ยกเลิก</Button
				><LoadingButton type="submit" loading={pending}>บันทึกชุดพัสดุ</LoadingButton>
			</div>
		</form>{:else}
		<div class="flex justify-end">
			<Button onclick={() => edit(null)} disabled={loading}>สร้างชุดพัสดุ</Button>
		</div>
		{#if loading && !result}<PageSkeleton variant="cards" rows={3} />{/if}
		{#if !loading && !result && error}<Button variant="outline" onclick={() => load()}
				>ลองอีกครั้ง</Button
			>{/if}
		{#if result}<div class="space-y-3" aria-busy={loading}>
				{#each result.items as bundle (bundle.id)}<Card.Root class="gap-0 py-0"
						><Card.Content class="space-y-3 p-4">
							<div class="flex flex-wrap items-center gap-2">
								<h3 class="min-w-0 flex-1 break-words font-medium">{bundle.name}</h3>
								<Badge variant="secondary">{bundle.shared ? 'ชุดส่วนกลาง' : 'ชุดส่วนตัว'}</Badge>
							</div>
							<ul class="space-y-1 text-sm">
								{#each bundle.entries as entry (entry.item.id)}<li
										class="flex justify-between gap-3"
									>
										<span class="min-w-0 break-words"
											>{entry.item.name} × {entry.quantity} {entry.item.unit}</span
										><span class="shrink-0 tabular-nums">{money(entry.subtotal)} ฿</span>
									</li>{/each}
							</ul>
							<div class="flex flex-wrap items-center justify-between gap-2">
								<p class="font-semibold">รวม {money(bundle.totalPrice)} ฿</p>
								{#if writable(bundle)}<div class="flex gap-1">
										<Button
											type="button"
											size="sm"
											variant="outline"
											disabled={pending}
											onclick={() => edit(bundle)}>แก้ไขชุด</Button
										><Button
											type="button"
											size="sm"
											variant="destructive"
											disabled={pending}
											onclick={() => (deleting = bundle)}>ลบชุด</Button
										>
									</div>{/if}
							</div>
						</Card.Content></Card.Root
					>{/each}{#if result.items.length === 0}<PageState
						title="ยังไม่มีชุดพัสดุ"
						description="สร้างชุดส่วนตัวเพื่อเก็บพัสดุและจำนวนที่ใช้บ่อย"
					/>{/if}
				<div class="flex items-center justify-between">
					<Button
						variant="outline"
						size="sm"
						disabled={loading || result.page === 1}
						onclick={() => load(result!.page - 1)}>ก่อนหน้า</Button
					><span class="text-sm text-muted-foreground">หน้า {result.page}</span><Button
						variant="outline"
						size="sm"
						disabled={loading || result.page * result.pageSize >= result.total}
						onclick={() => load(result!.page + 1)}>ถัดไป</Button
					>
				</div>
			</div>{/if}
	{/if}
</div>
<AlertDialog.Root
	open={!!deleting}
	onOpenChange={(open) => {
		if (!open && !pending) deleting = null;
	}}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ลบชุด {deleting?.name}?</AlertDialog.Title><AlertDialog.Description
				>รายการพัสดุในคลังจะยังอยู่ แต่ชุดนี้จะถูกลบถาวร</AlertDialog.Description
			></AlertDialog.Header
		><AlertDialog.Footer
			><Button variant="outline" disabled={pending} onclick={() => (deleting = null)}>ยกเลิก</Button
			><LoadingButton variant="destructive" loading={pending} onclick={remove}
				>ลบชุดพัสดุ</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
