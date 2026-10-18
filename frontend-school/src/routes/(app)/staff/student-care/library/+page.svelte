<script lang="ts">
	import { LibraryBig } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, respondentLabels } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careInstruments } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับภาพรวมการดูแล"
	icon={LibraryBig}
	title="คลังแบบประเมิน"
	backHref={careLink('', data.term)}
	>{#snippet actions()}{#if $can.hasAny(...[PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL])}<Button
				href={careLink('/library/new', data.term)}>เพิ่มแบบประเมิน</Button
			>{/if}{/snippet}<CareNavigation
		current="library"
		term={data.term}
	/>{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careReadPermissions}
			retry={(signal) => careInstruments(data.query, { signal })}
			label="คลังแบบประเมิน"
			>{#snippet children(items)}{#if !items.length}<PageState
						title={data.query.afterId ? 'ไม่พบแบบประเมินในรายการนี้' : 'ยังไม่มีแบบประเมิน'}
						description={data.query.afterId
							? 'กลับหน้าแรกเพื่อดูแบบประเมินอื่นในขอบเขตของคุณ'
							: 'ผู้มีสิทธิ์จัดการคลังสามารถเพิ่มแบบประเมินจากปุ่มด้านบน โดยใช้เนื้อหาและเกณฑ์ที่ได้รับอนุญาต'}
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.definition.name}</p>
									<p class="text-sm text-muted-foreground">
										{item.definition.variant} · รุ่น {item.version} · {respondentLabels[
											item.definition.respondent
										]}
									</p>
									<p class="text-sm text-muted-foreground">
										{item.definition.mode === 'digital' ? 'ออนไลน์' : 'บันทึกคะแนนกระดาษ'} · {item
											.definition.minimumAge}–{item.definition.maximumAge} ปี · {item.definition
											.language}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/library/${item.id}`, data.term)}
									>ดูแบบประเมิน</Button
								>
							</li>{/each}
					</ul>{/if}<CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
