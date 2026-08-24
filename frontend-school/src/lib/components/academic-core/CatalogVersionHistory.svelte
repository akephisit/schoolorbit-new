<script module lang="ts">
	export type CatalogVersionItem = {
		id: string;
		versionNo: number;
		name: string;
		secondaryName?: string | null;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil?: string | null;
		status: 'draft' | 'published' | 'archived';
		rowVersion: number;
	};

	export type CatalogVersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};
</script>

<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { CheckCircle2, GitBranchPlus, History } from 'lucide-svelte';

	let {
		kind,
		code,
		items,
		canManage = false,
		onCreate,
		onPublish
	}: {
		kind: 'subject' | 'activity';
		code: string;
		items: CatalogVersionItem[];
		canManage?: boolean;
		onCreate: (draft: CatalogVersionDraft) => Promise<void>;
		onPublish: (id: string, rowVersion: number) => Promise<void>;
	} = $props();

	let draft = $state<CatalogVersionDraft>({
		name: '',
		secondaryName: '',
		exactValue: '1.00',
		effectiveFrom: '',
		effectiveUntil: '',
		gradeLevelIds: [],
		classification: ''
	});
	let gradeLevelsText = $state('');
	let busy = $state(false);
	let errorMessage = $state('');

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onCreate({
				...draft,
				gradeLevelIds: gradeLevelsText
					.split(',')
					.map((value) => value.trim())
					.filter(Boolean)
			});
			draft = { ...draft, name: '', secondaryName: '', effectiveUntil: '' };
			gradeLevelsText = '';
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างรุ่นใหม่ไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function publish(item: CatalogVersionItem) {
		busy = true;
		errorMessage = '';
		try {
			await onPublish(item.id, item.rowVersion);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
	<section class="rounded-xl border bg-card">
		<header class="flex items-center gap-2 border-b px-5 py-4">
			<History class="size-5 text-primary" />
			<div>
				<h2 class="font-semibold">ประวัติรุ่น · {code}</h2>
				<p class="text-xs text-muted-foreground">รุ่นที่เผยแพร่แล้วจะไม่ถูกแก้ทับ</p>
			</div>
		</header>
		<div class="divide-y">
			{#each items as item (item.id)}
				<article class="grid gap-3 px-5 py-4 sm:grid-cols-[52px_1fr_auto] sm:items-center">
					<div class="text-center">
						<p class="text-lg font-semibold tabular-nums">v{item.versionNo}</p>
						<p class="text-[10px] uppercase text-muted-foreground">version</p>
					</div>
					<div>
						<div class="flex flex-wrap items-center gap-2">
							<h3 class="font-medium">{item.name}</h3>
							<Badge variant={item.status === 'published' ? 'default' : 'secondary'}
								>{item.status}</Badge
							>
						</div>
						{#if item.secondaryName}<p class="text-xs text-muted-foreground">
								{item.secondaryName}
							</p>{/if}
						<p class="mt-1 text-xs text-muted-foreground">
							{item.exactValue} · เริ่ม {item.effectiveFrom}{item.effectiveUntil
								? ` ถึง ${item.effectiveUntil}`
								: ''}
						</p>
					</div>
					{#if canManage && item.status === 'draft'}<Button
							size="sm"
							variant="outline"
							disabled={busy}
							onclick={() => publish(item)}><CheckCircle2 class="size-4" /> เผยแพร่</Button
						>{/if}
				</article>
			{:else}
				<p class="p-8 text-center text-sm text-muted-foreground">ยังไม่มีรุ่นข้อมูล</p>
			{/each}
		</div>
	</section>

	{#if canManage}
		<form class="space-y-3 rounded-xl border bg-card p-5" onsubmit={submit}>
			<div class="flex items-center gap-2">
				<GitBranchPlus class="size-5 text-primary" />
				<h2 class="font-semibold">สร้างรุ่นใหม่</h2>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-name`}>ชื่อภาษาไทย</Label><Input
					id={`${code}-version-name`}
					bind:value={draft.name}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-en`}>ชื่อภาษาอังกฤษ</Label><Input
					id={`${code}-version-en`}
					bind:value={draft.secondaryName}
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-exact`}
					>{kind === 'subject' ? 'หน่วยกิต' : 'ชั่วโมงต่อสัปดาห์'}</Label
				><Input
					id={`${code}-version-exact`}
					inputmode="decimal"
					bind:value={draft.exactValue}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-class`}
					>{kind === 'subject' ? 'ประเภทรายวิชา' : 'รูปแบบจัดกิจกรรม'}</Label
				><Input id={`${code}-version-class`} bind:value={draft.classification} required />
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-levels`}>รหัสระดับชั้น (คั่นด้วยจุลภาค)</Label><Input
					id={`${code}-version-levels`}
					bind:value={gradeLevelsText}
					required
				/>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1.5">
					<Label for={`${code}-version-from`}>เริ่มใช้</Label><Input
						id={`${code}-version-from`}
						type="date"
						bind:value={draft.effectiveFrom}
						required
					/>
				</div>
				<div class="space-y-1.5">
					<Label for={`${code}-version-until`}>สิ้นสุด</Label><Input
						id={`${code}-version-until`}
						type="date"
						bind:value={draft.effectiveUntil}
					/>
				</div>
			</div>
			<Button class="w-full" type="submit" disabled={busy}
				><GitBranchPlus class="size-4" /> บันทึกร่างรุ่นใหม่</Button
			>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</form>
	{/if}
</div>
