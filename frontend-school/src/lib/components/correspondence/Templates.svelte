<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		listProcessTemplates,
		getCorrespondenceOptions,
		type ProcessTemplate,
		type CorrespondenceOptions
	} from '#lib/api/correspondence.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import TemplateEditor from './TemplateEditor.svelte';
	let { source }: { source: Promise<RouteLoadResult<ProcessTemplate[]>> } = $props();
	const region = new CorrespondenceRegion<ProcessTemplate[]>();
	let editing = $state(false),
		selected = $state<ProcessTemplate | undefined>(),
		options = $state<CorrespondenceOptions | null>(null),
		optionsError = $state(''),
		optionsLoading = $state(false);
	let disposed = false;
	const controller = new AbortController();
	$effect.pre(() => {
		const operation = source;
		untrack(() => region.consume(operation));
	});
	onDestroy(() => {
		disposed = true;
		controller.abort();
		region.dispose();
	});
	async function open(template?: ProcessTemplate) {
		editing = true;
		selected = template;
		await loadOptions();
	}
	async function loadOptions() {
		optionsLoading = true;
		optionsError = '';
		try {
			const result = await getCorrespondenceOptions('', { signal: controller.signal });
			if (!disposed) options = result;
		} catch (cause) {
			if (!disposed)
				optionsError = cause instanceof Error ? cause.message : 'โหลดตัวเลือกไม่สำเร็จ';
		} finally {
			if (!disposed) optionsLoading = false;
		}
	}
</script>

<PageShell
	title="แม่แบบการดำเนินงาน"
	backHref="/staff/correspondence"
	description="กำหนดขั้นตอน ผู้รับ และเงื่อนไขมาตรฐานของหนังสือ"
>
	{#snippet actions()}<Button onclick={() => open()}>สร้างแม่แบบ</Button>{/snippet}
	{#if editing}<Card.Root
			><Card.Header><Card.Title>{selected ? 'แก้ไขแม่แบบ' : 'สร้างแม่แบบ'}</Card.Title></Card.Header
			><Card.Content
				>{#if optionsError}<PageState
						variant="error"
						title={optionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadOptions}
					/>{/if}{#if optionsLoading && !options}<PageSkeleton
					/>{:else if options}{#key selected?.id ?? 'new'}<TemplateEditor
							template={selected}
							{options}
							oncancel={() => {
								editing = false;
							}}
							onsaved={(saved) => {
								region.patch([
									saved,
									...(region.value ?? []).filter((item) => item.id !== saved.id)
								]);
								editing = false;
							}}
						/>{/key}{/if}</Card.Content
			></Card.Root
		>{/if}
	<section aria-busy={region.loading} data-testid="process-templates">
		{#if region.error}<PageState
				variant="error"
				title={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listProcessTemplates({ signal }))}
			/>{/if}{#if region.loading && !region.value}<PageSkeleton
			/>{:else if region.value?.length}<div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
				{#each region.value as template (template.id)}<Card.Root
						><Card.Header
							><Card.Title>{template.title}</Card.Title><Card.Description
								>รุ่น {template.version} · {template.steps.length} ขั้นตอน · {template.active
									? 'เปิดใช้งาน'
									: 'ปิดใช้งาน'}</Card.Description
							></Card.Header
						><Card.Content
							><ol class="mb-4 list-inside list-decimal space-y-2 text-sm">
								{#each template.steps as step, index (index)}<li>{step.title}</li>{/each}
							</ol>
							<Button variant="outline" onclick={() => open(template)}>แก้ไขแม่แบบ</Button
							></Card.Content
						></Card.Root
					>{/each}
			</div>{:else if !region.error}<PageState
				title="ยังไม่มีแม่แบบ"
				description="สร้างแม่แบบเพื่อให้ผู้ส่งเลือกใช้กับหนังสือ"
			/>{/if}
	</section>
</PageShell>
