<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Dialog from '$lib/components/ui/dialog';
	import { can, userPermissions } from '$lib/stores/permissions';
	import { appIdentityKey } from '$lib/auth/settled-user';
	import { authStore } from '$lib/stores/auth';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import {
		listStaffReferenceItems,
		createStaffReferenceItem,
		updateStaffReferenceItem,
		type ReferencePage,
		type StaffReferenceItem
	} from '$lib/api/personnel';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let catalog = $state<ReferencePage | null>(null),
		loading = $state(true),
		error = $state(''),
		search = $state('');
	let dialog = $state(false),
		editing = $state<StaffReferenceItem | null>(null),
		name = $state(''),
		saving = $state(false),
		saveError = $state('');
	const request = new LatestRequest();
	const identityKey = $derived.by(() => {
		void $authStore.user;
		void $userPermissions;
		return appIdentityKey();
	});
	let sourceOwner = '';
	let previousSource: typeof data.catalog | undefined;
	let context = '',
		epoch = 0,
		disposed = false;
	const allowed = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const kinds = {
		job_position: 'ตำแหน่งงาน',
		major: 'สาขาวิชา',
		university: 'สถาบันการศึกษา'
	} as const;
	$effect.pre(() => {
		const key = `${identityKey}:${data.contextKey}`,
			source = data.catalog;
		untrack(() => {
			if (key !== context) {
				context = key;
				catalog = null;
				epoch++;
				dialog = false;
				saving = false;
				search = data.query.search ?? '';
			}
			if (!identityKey || (source === previousSource && sourceOwner !== identityKey)) {
				request.abort();
				loading = Boolean(identityKey);
				return;
			}
			previousSource = source;
			sourceOwner = identityKey;
			const ticket = request.begin();
			loading = true;
			error = '';
			void source.then((result) => apply(result, ticket.revision));
		});
		return () => request.abort();
	});
	$effect(() => {
		if (!allowed) {
			request.abort();
			catalog = null;
			dialog = false;
			epoch++;
			saving = false;
		}
	});
	onDestroy(() => {
		disposed = true;
		epoch++;
		request.abort();
	});
	function apply(result: Awaited<typeof data.catalog>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) catalog = result.data;
		else error = result.error;
	}
	async function refresh() {
		if (!allowed) return;
		const ticket = request.begin();
		loading = true;
		error = '';
		apply(
			await captureRouteLoad(
				listStaffReferenceItems(data.query, { signal: ticket.signal }),
				'โหลดรายการกลางไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	function navigate(key: string, value: string) {
		const query = new SvelteURLSearchParams(page.url.search);
		query.set(key, value);
		if (key !== 'page') query.delete('page');
		void goto(resolve(`/staff/manage/reference-data?${query}`));
	}
	function openEditor(item: StaffReferenceItem | null) {
		editing = item;
		name = item?.name ?? '';
		saveError = '';
		dialog = true;
		epoch++;
	}
	async function save(active?: boolean) {
		if (!allowed || saving || !dialog) return;
		const owner = context,
			revision = ++epoch,
			id = editing?.id;
		const draft = name.trim();
		saving = true;
		saveError = '';
		try {
			const item = id
				? await updateStaffReferenceItem(id, { name: draft, isActive: active })
				: await createStaffReferenceItem({ kind: data.query.kind, name: draft });
			if (disposed || owner !== context || revision !== epoch || !dialog || !allowed) return;
			if (catalog) {
				const exists = catalog.items.some((row) => row.id === item.id);
				const matches =
					(data.query.status === 'all' || item.isActive === (data.query.status !== 'inactive')) &&
					(!data.query.search ||
						item.name.toLocaleLowerCase().includes(data.query.search.toLocaleLowerCase()));
				const rows = catalog.items.filter((row) => row.id !== item.id);
				if (matches) rows.push(item);
				catalog = {
					...catalog,
					items: rows
						.sort((a, b) => a.displayOrder - b.displayOrder || a.name.localeCompare(b.name, 'th'))
						.slice(0, 25),
					total: catalog.total + (exists ? (matches ? 0 : -1) : matches ? 1 : 0)
				};
			}
			dialog = false;
		} catch (cause) {
			if (!disposed && owner === context && revision === epoch && dialog)
				saveError = cause instanceof Error ? cause.message : 'บันทึกรายการไม่สำเร็จ';
		} finally {
			if (revision === epoch) saving = false;
		}
	}
</script>

<PageShell
	title="รายการกลางงานบุคคล"
	description="ตำแหน่ง สาขาวิชา และสถาบันที่ใช้ร่วมกันในแบบฟอร์มบุคลากร"
>
	<div class="flex flex-wrap items-center justify-between gap-3">
		<a href={resolve('/staff/manage')} class="text-sm text-primary underline">กลับรายชื่อบุคลากร</a
		><Button onclick={() => openEditor(null)} disabled={!allowed}>เพิ่มรายการ</Button>
	</div>
	<form
		class="rounded-xl border bg-card p-3 sm:p-4 flex flex-wrap gap-3"
		onsubmit={(e) => {
			e.preventDefault();
			navigate('search', search);
		}}
	>
		<Select.Root type="single" value={data.query.kind} onValueChange={(v) => navigate('kind', v)}
			><Select.Trigger aria-label="ชนิดรายการ" class="w-full sm:w-48"
				>{kinds[data.query.kind]}</Select.Trigger
			><Select.Content
				>{#each Object.entries(kinds) as [key, label] (key)}<Select.Item value={key}
						>{label}</Select.Item
					>{/each}</Select.Content
			></Select.Root
		>
		<Input
			class="w-full sm:w-64"
			aria-label="ค้นหารายการ"
			placeholder="ค้นหาชื่อรายการ"
			bind:value={search}
		/><Button type="submit" variant="outline">ค้นหา</Button>
		<Select.Root
			type="single"
			value={data.query.status ?? 'active'}
			onValueChange={(v) => navigate('status', v)}
			><Select.Trigger aria-label="สถานะรายการ" class="w-full sm:w-40"
				>{data.query.status === 'all'
					? 'ทุกรายการ'
					: data.query.status === 'inactive'
						? 'ปิดใช้งาน'
						: 'ใช้งาน'}</Select.Trigger
			><Select.Content
				><Select.Item value="active">ใช้งาน</Select.Item><Select.Item value="inactive"
					>ปิดใช้งาน</Select.Item
				><Select.Item value="all">ทุกรายการ</Select.Item></Select.Content
			></Select.Root
		>
	</form>
	<section data-testid="staff-reference-catalog" aria-busy={loading} class="space-y-3">
		{#if error}<PageState
				variant="error"
				title={error}
				actionLabel="ลองอีกครั้ง"
				onaction={refresh}
			/>{/if}
		{#if !catalog && loading}<PageSkeleton variant="table" rows={5} />{:else if catalog}
			<div class="flex justify-between gap-3 text-sm">
				<p>
					{catalog.total} รายการ {#if loading}<span role="status">กำลังอัปเดต…</span>{/if}
				</p>
				<Button variant="outline" size="sm" onclick={refresh} disabled={loading}>รีเฟรช</Button>
			</div>
			<div class="overflow-x-auto rounded-xl border">
				<table class="w-full text-sm">
					<thead class="bg-muted"
						><tr
							><th class="p-3 text-left">ชื่อรายการ</th><th class="p-3 text-left">สถานะ</th><th
								class="p-3 text-right">จัดการ</th
							></tr
						></thead
					><tbody
						>{#each catalog.items as item (item.id)}<tr class="border-t"
								><td class="p-3 break-words">{item.name}</td><td class="p-3 whitespace-nowrap"
									>{item.isActive ? 'ใช้งาน' : 'ปิดใช้งาน'}</td
								><td class="p-3 text-right"
									><Button
										size="sm"
										variant="outline"
										onclick={() => openEditor(item)}
										disabled={!allowed}>แก้ไข</Button
									></td
								></tr
							>{:else}<tr
								><td colspan="3" class="p-6 text-center text-muted-foreground">ไม่พบรายการ</td></tr
							>{/each}</tbody
					>
				</table>
			</div>
			<div class="flex items-center justify-end gap-3">
				<Button
					variant="outline"
					disabled={catalog.page <= 1}
					onclick={() => navigate('page', String(catalog!.page - 1))}>ก่อนหน้า</Button
				><span class="text-sm">หน้า {catalog.page}</span><Button
					variant="outline"
					disabled={catalog.page * catalog.pageSize >= catalog.total}
					onclick={() => navigate('page', String(catalog!.page + 1))}>ถัดไป</Button
				>
			</div>
		{/if}
	</section>
	<Dialog.Root
		bind:open={dialog}
		onOpenChange={(open) => {
			if (!open) {
				epoch++;
				saving = false;
			}
		}}
		><Dialog.Content
			><Dialog.Header
				><Dialog.Title>{editing ? 'แก้ไขรายการ' : 'เพิ่มรายการ'}</Dialog.Title><Dialog.Description
					>ใช้ชื่อเดียวกันเพื่อให้ค้นหาและสรุปข้อมูลได้ตรงกัน</Dialog.Description
				></Dialog.Header
			>
			<form
				class="space-y-4"
				onsubmit={(e) => {
					e.preventDefault();
					void save();
				}}
			>
				<label for="reference-name" class="text-sm font-medium">ชื่อรายการ</label><Input
					id="reference-name"
					bind:value={name}
					maxlength={200}
					required
					disabled={saving}
				/>{#if saveError}<p role="alert" class="text-sm text-destructive">
						{saveError}
					</p>{/if}<Dialog.Footer
					>{#if editing}<Button
							type="button"
							variant="outline"
							disabled={saving}
							onclick={() => save(!editing!.isActive)}
							>{editing.isActive ? 'ปิดใช้งาน' : 'เปิดใช้งาน'}</Button
						>{/if}<Button type="submit" disabled={saving || !name.trim()}
						>{saving ? 'กำลังบันทึก…' : 'บันทึกรายการ'}</Button
					></Dialog.Footer
				>
			</form></Dialog.Content
		></Dialog.Root
	>
</PageShell>
