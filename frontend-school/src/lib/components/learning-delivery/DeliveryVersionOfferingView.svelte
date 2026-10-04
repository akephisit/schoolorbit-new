<script lang="ts">
	import type { DeliveryVersion } from '#lib/api/learning-delivery.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	let { version, offeringId }: { version: DeliveryVersion; offeringId: string } = $props();
	const offering = $derived(version.snapshot.offerings.find((item) => item.id === offeringId));
	const roleNames = { primary: 'ครูหลัก', secondary: 'ครูร่วม', assistant: 'ครูผู้ช่วย' };
</script>

{#if offering}
	<section
		class="space-y-4 rounded-xl border bg-card p-4 sm:p-5"
		data-testid="delivery-version-offering"
	>
		<div class="flex flex-wrap items-start justify-between gap-3">
			<div>
				<h2 class="text-lg font-semibold">{offering.code} · {offering.name}</h2>
				<p class="mt-1 text-sm text-muted-foreground">
					รุ่นเปิดสอนเริ่มใช้ {version.effectiveFrom}{version.effectiveUntil
						? ` ถึง ${version.effectiveUntil}`
						: ''}
				</p>
			</div>
			<Badge variant="secondary">เผยแพร่แล้ว</Badge>
		</div>
		<p class="text-sm">
			เปิดสอน {offering.weeklyPeriodTarget} คาบ/สัปดาห์ · {offering.groups.length} กลุ่มเรียน
		</p>
		<p class="text-sm text-muted-foreground">
			ข้อมูลในรุ่นนี้เก็บไว้สำหรับอ้างอิงและดูย้อนหลัง หากต้องการแก้ไข
			ให้สร้างรุ่นเปิดสอนใหม่จากหน้าภาพรวม
		</p>
		<div class="grid gap-3 md:grid-cols-2">
			{#each offering.groups as group (group.id)}
				<article class="rounded-lg border p-4">
					<h3 class="font-medium">{group.code} · {group.name}</h3>
					{#if group.description}<p class="mt-1 text-sm text-muted-foreground">
							{group.description}
						</p>{/if}
					<p class="mt-2 text-sm">
						ห้องต้นทาง {group.homeroomIds.length} ห้อง{group.capacity !== null
							? ` · รับได้ ${group.capacity} คน`
							: ''}
					</p>
					<ul class="mt-2 space-y-1 text-sm">
						{#each group.teachers as teacher (teacher.assignmentId)}
							<li>{teacher.displayName} · {roleNames[teacher.role]}</li>
						{/each}
					</ul>
				</article>
			{/each}
		</div>
	</section>
{:else}
	<PageState
		variant="empty"
		title="รายการนี้ไม่อยู่ในรุ่นเปิดสอนที่เลือก"
		description="กลับหน้าภาพรวมเพื่อเลือกรุ่นเปิดสอนที่ต้องการดู"
	/>
{/if}
