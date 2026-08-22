<script lang="ts">
	import { onMount } from 'svelte';
	import { ApiClientError } from '$lib/api/client';
	import {
		attachSchoolFontBatch,
		deleteSchoolFont,
		inspectSchoolFontUploads,
		listSchoolFonts,
		type SchoolFontDeleteConflict,
		type SchoolFontSummary
	} from '$lib/api/school-fonts';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Library, RefreshCw, Trash2, Type } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import SchoolFontBatchUpload from './SchoolFontBatchUpload.svelte';

	let fonts = $state.raw<SchoolFontSummary[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let refreshing = $state(false);
	let uploadPending = $state(false);
	let deleteTarget = $state.raw<SchoolFontSummary | null>(null);
	let deletingId = $state<string | null>(null);
	let actionError = $state<string | null>(null);

	const totalReferences = $derived(fonts.reduce((total, font) => total + font.referenceCount, 0));
	const families = $derived.by(() => {
		const grouped: Record<string, SchoolFontSummary[]> = {};
		for (const font of fonts) {
			grouped[font.fontFamily] = [...(grouped[font.fontFamily] ?? []), font];
		}
		return Object.entries(grouped)
			.map(([family, variants]) => ({
				family,
				variants: variants.toSorted(compareFonts)
			}))
			.toSorted((left, right) => left.family.localeCompare(right.family, 'th'));
	});

	function compareFonts(left: SchoolFontSummary, right: SchoolFontSummary): number {
		return (
			left.fontFamily.localeCompare(right.fontFamily, 'th') ||
			left.fontWeight - right.fontWeight ||
			left.fontStyle.localeCompare(right.fontStyle) ||
			left.id.localeCompare(right.id)
		);
	}

	function asMessage(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}

	async function loadFonts(refresh = false): Promise<void> {
		if (refresh) refreshing = true;
		else loading = true;
		loadError = null;
		try {
			const result = await listSchoolFonts();
			fonts = result.items.toSorted(compareFonts);
		} catch (error) {
			loadError = asMessage(error, 'โหลดคลังฟอนต์ไม่สำเร็จ');
		} finally {
			loading = false;
			refreshing = false;
		}
	}

	function patchAttached(items: SchoolFontSummary[]): void {
		const attachedIds = new Set(items.map((font) => font.id));
		fonts = [...fonts.filter((font) => !attachedIds.has(font.id)), ...items].toSorted(compareFonts);
		actionError = null;
	}

	function schoolFontDeleteConflict(error: unknown): SchoolFontDeleteConflict | null {
		if (!(error instanceof ApiClientError) || error.status !== 409) return null;
		const data = error.data;
		if (!data || typeof data !== 'object' || !('referenceCount' in data)) return null;
		return typeof data.referenceCount === 'number' ? { referenceCount: data.referenceCount } : null;
	}

	async function confirmDelete(): Promise<void> {
		const target = deleteTarget;
		if (!target || deletingId) return;
		deletingId = target.id;
		actionError = null;
		try {
			await deleteSchoolFont(target.id);
			fonts = fonts.filter((font) => font.id !== target.id);
			deleteTarget = null;
			toast.success(`ลบ ${target.displayName} จากคลังแล้ว`);
		} catch (error) {
			const conflict = schoolFontDeleteConflict(error);
			if (conflict) {
				const referenceCount = conflict.referenceCount;
				fonts = fonts.map((font) => (font.id === target.id ? { ...font, referenceCount } : font));
				actionError = `ฟอนต์นี้ยังถูกใช้ใน ${referenceCount} แม่แบบ`;
			} else {
				actionError = asMessage(error, 'ลบฟอนต์ไม่สำเร็จ');
			}
			deleteTarget = null;
		} finally {
			deletingId = null;
		}
	}

	onMount(() => {
		void loadFonts();
	});
</script>

<div class="space-y-5" data-testid="school-font-library">
	<div class="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)]">
		<Card>
			<CardHeader>
				<CardTitle>รับฟอนต์เข้าคลัง</CardTitle>
				<CardDescription>
					ตรวจ variant และยืนยันสิทธิ์เป็นชุด ก่อนเปิดให้ทุกระบบของโรงเรียนใช้ร่วมกัน
				</CardDescription>
			</CardHeader>
			<CardContent>
				<SchoolFontBatchUpload
					context={{ type: 'central' }}
					inspectUploads={inspectSchoolFontUploads}
					attachBatch={attachSchoolFontBatch}
					onattached={patchAttached}
					onpendingchange={(pending) => (uploadPending = pending)}
				/>
			</CardContent>
		</Card>

		<Card>
			<CardHeader class="gap-3 sm:flex-row sm:items-start sm:justify-between">
				<div class="space-y-1.5">
					<div class="flex items-center gap-2">
						<span class="grid size-9 place-items-center rounded-xl bg-slate-900 text-white">
							<Library class="size-4" />
						</span>
						<CardTitle>ตู้แบบอักษรของโรงเรียน</CardTitle>
					</div>
					<CardDescription>
						จัดเรียงตาม family และ variant เพื่อให้ผู้จัดทำงานทุกคนเลือกใช้ชื่อเดียวกัน
					</CardDescription>
				</div>
				<Button
					variant="outline"
					size="sm"
					onclick={() => loadFonts(true)}
					disabled={refreshing || loading || uploadPending}
				>
					<RefreshCw class={refreshing ? 'size-4 animate-spin' : 'size-4'} /> รีเฟรช
				</Button>
			</CardHeader>
			<CardContent class="space-y-4">
				<div class="grid grid-cols-3 divide-x rounded-xl border bg-muted/20 py-3 text-center">
					<div>
						<p class="text-lg font-semibold tabular-nums">{families.length}</p>
						<p class="text-[11px] text-muted-foreground">family</p>
					</div>
					<div>
						<p class="text-lg font-semibold tabular-nums">{fonts.length}</p>
						<p class="text-[11px] text-muted-foreground">variant</p>
					</div>
					<div>
						<p class="text-lg font-semibold tabular-nums">{totalReferences}</p>
						<p class="text-[11px] text-muted-foreground">การใช้งาน</p>
					</div>
				</div>

				{#if actionError}
					<div
						class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
						role="alert"
					>
						{actionError}
					</div>
				{/if}

				{#if loading}
					<PageSkeleton variant="detail" rows={4} />
				{:else if loadError}
					<PageState
						variant="error"
						title="โหลดคลังฟอนต์ไม่สำเร็จ"
						description={loadError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => loadFonts()}
					/>
				{:else if families.length === 0}
					<PageState
						title="คลังฟอนต์ยังว่าง"
						description="เลือกไฟล์ทางซ้ายเพื่อตรวจและเพิ่มฟอนต์ชุดแรกของโรงเรียน"
					/>
				{:else}
					<div class="space-y-3">
						{#each families as group (group.family)}
							<article class="overflow-hidden rounded-xl border">
								<header class="flex items-center justify-between gap-3 bg-muted/35 px-3 py-2.5">
									<div class="flex min-w-0 items-center gap-2">
										<Type class="size-4 shrink-0 text-violet-600" />
										<h3 class="truncate text-sm font-semibold">{group.family}</h3>
									</div>
									<Badge variant="secondary">{group.variants.length} variant</Badge>
								</header>
								<div class="divide-y">
									{#each group.variants as font (font.id)}
										<div
											class="grid gap-3 px-3 py-3 sm:grid-cols-[4.5rem_minmax(0,1fr)_auto] sm:items-center"
										>
											<div
												class="rounded-lg border bg-background px-2 py-1.5 text-center font-mono text-sm font-semibold tabular-nums"
											>
												{font.fontWeight}
											</div>
											<div class="min-w-0">
												<p class="truncate text-sm font-medium">{font.displayName}</p>
												<div class="mt-1 flex flex-wrap items-center gap-1.5">
													<Badge variant="outline">
														{font.fontStyle === 'italic' ? 'ตัวเอียง' : 'ตัวตรง'}
													</Badge>
													<span class="text-xs text-muted-foreground">
														{font.referenceCount === 0
															? 'ยังไม่มีแม่แบบใช้งาน'
															: `ใช้ใน ${font.referenceCount} แม่แบบ`}
													</span>
												</div>
											</div>
											<Button
												size="icon-sm"
												variant="ghost"
												onclick={() => (deleteTarget = font)}
												disabled={deletingId === font.id}
												aria-label={`ลบฟอนต์ ${font.displayName}`}
											>
												<Trash2 class="size-4" />
											</Button>
										</div>
									{/each}
								</div>
							</article>
						{/each}
					</div>
				{/if}
			</CardContent>
		</Card>
	</div>
</div>

<AlertDialog.Root
	open={deleteTarget !== null}
	onOpenChange={(open) => !open && (deleteTarget = null)}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ลบ “{deleteTarget?.displayName ?? ''}” ออกจากคลัง?</AlertDialog.Title>
			<AlertDialog.Description>
				ลบได้เฉพาะฟอนต์ที่ไม่มีแม่แบบอ้างอิง ระบบจะตรวจจำนวนใช้งานล่าสุดอีกครั้งก่อนลบ
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={deletingId !== null}>ยกเลิก</AlertDialog.Cancel>
			<AlertDialog.Action
				onclick={confirmDelete}
				disabled={deletingId !== null}
				class="bg-destructive text-white"
			>
				{deletingId ? 'กำลังลบ...' : 'ยืนยันลบฟอนต์'}
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
