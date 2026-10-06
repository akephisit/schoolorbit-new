<script lang="ts">
	import type { CurriculumOverviewItem } from '#lib/api/academic-core.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	let { items }: { items: CurriculumOverviewItem[] } = $props();
</script>

<div class="overflow-x-auto rounded-xl border">
	<Table.Root>
		<Table.Header
			><Table.Row
				><Table.Head>ฉบับหลักสูตร</Table.Head><Table.Head>ปีปรับปรุง</Table.Head><Table.Head
					>สถานะ</Table.Head
				><Table.Head>ระดับการศึกษา</Table.Head><Table.Head>แผนการเรียน</Table.Head></Table.Row
			></Table.Header
		>
		<Table.Body
			>{#each items as item (item.edition.id)}<Table.Row>
					<Table.Cell
						><a
							class="font-medium text-primary underline-offset-4 hover:underline"
							href={`/staff/academic/curricula/${item.edition.id}`}>{item.edition.name}</a
						></Table.Cell
					>
					<Table.Cell>{item.edition.revisionYear ?? 'ยังไม่กำหนด'}</Table.Cell>
					<Table.Cell
						><Badge variant={item.edition.status === 'published' ? 'default' : 'secondary'}
							>{item.edition.status === 'published'
								? 'เผยแพร่แล้ว'
								: item.edition.status === 'draft'
									? 'ฉบับร่าง'
									: 'เก็บถาวร'}</Badge
						></Table.Cell
					>
					<Table.Cell>{item.levelCount}</Table.Cell><Table.Cell>{item.studyProgramCount}</Table.Cell
					>
				</Table.Row>{/each}</Table.Body
		>
	</Table.Root>
</div>
