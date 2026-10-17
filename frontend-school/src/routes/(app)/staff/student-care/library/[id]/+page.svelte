<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, respondentLabels } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CareSection from '#lib/components/student-care/CareSection.svelte';
	import { careInstrument } from '#lib/api/student-care.js';
	import { familyLabels } from '#lib/features/student-care/presentation.js';
	let { data }: PageProps = $props();
</script>

<PageShell title="รายละเอียดแบบประเมิน" backHref={careLink('/library', data.term)}
	>{#snippet actions()}{#if $can.has(PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL)}<Button
				href={`/staff/student-care/library/new?fromVersionId=${data.id}`}
				>สร้างรุ่นใหม่จากรุ่นนี้</Button
			>{/if}{/snippet}{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careReadPermissions}
			retry={(signal) => careInstrument(data.id, { signal })}
			variant="detail"
			>{#snippet children(item)}<CareSection
					title={item.definition.name}
					description={`${familyLabels[item.definition.family]} · ${item.definition.variant} · รุ่น ${item.version}`}
					><dl class="grid gap-3 text-sm sm:grid-cols-2">
						<div>
							<dt class="text-muted-foreground">ผู้ตอบ</dt>
							<dd>{respondentLabels[item.definition.respondent]}</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">อายุ</dt>
							<dd>{item.definition.minimumAge}–{item.definition.maximumAge} ปี</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">วิธีประเมิน</dt>
							<dd>{item.definition.mode === 'digital' ? 'ออนไลน์' : 'บันทึกคะแนนกระดาษ'}</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">ภาษา</dt>
							<dd>{item.definition.language}</dd>
						</div>
					</dl>
					<p class="whitespace-pre-wrap text-sm">{item.definition.instructions}</p>
					<div>
						<h3 class="text-sm font-medium">แหล่งที่มา</h3>
						<p class="whitespace-pre-wrap break-words text-sm text-muted-foreground">
							{item.definition.source}
						</p>
					</div>
					<div>
						<h3 class="text-sm font-medium">เงื่อนไขการใช้งาน</h3>
						<p class="whitespace-pre-wrap break-words text-sm text-muted-foreground">
							{item.definition.licenseNote}
						</p>
					</div></CareSection
				><CareSection title="มาตรวัดและเกณฑ์"
					>{#each item.definition.scales as scale (scale.key)}<section
							class="space-y-2 border-b pb-3 last:border-0"
						>
							<h3 class="font-medium">{scale.label}</h3>
							{#if scale.sourceScaleKeys.length}<p class="text-xs text-muted-foreground">
									รวมจาก {scale.sourceScaleKeys
										.map((key) => item.definition.scales.find((s) => s.key === key)?.label ?? key)
										.join(', ')}
								</p>{/if}
							<ul class="space-y-1 text-sm">
								{#each scale.bands as band (band.key)}<li>
										{band.minimum}–{band.maximum}: {band.label}{band.urgent
											? ' · ติดตามเร่งด่วน'
											: ''}
									</li>{/each}
							</ul>
						</section>{/each}</CareSection
				>{/snippet}</CareRegion
		>{/key}</PageShell
>
