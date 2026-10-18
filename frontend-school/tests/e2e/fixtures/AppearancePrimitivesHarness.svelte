<script lang="ts">
	import { Table, TableBody, TableRow, TableCell } from '#lib/components/ui/table/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Card } from '#lib/components/ui/card/index.js';
</script>

<main>
	<p class="font-sans" data-testid="font-sans-consumer">แบบอักษรของโรงเรียน</p>
	<Button data-testid="primary-button">บันทึก</Button>
	<Button data-testid="destructive-button" variant="destructive">ลบ</Button>
	<Badge data-testid="destructive-badge" variant="destructive" href="#example">เกินกำหนด</Badge>
	<Table
		><TableBody
			><TableRow
				><TableCell data-testid="default-cell">ข้อมูลตัวอย่าง</TableCell><TableCell
					data-testid="explicit-cell"
					class="py-3">กำหนดระยะเฉพาะ</TableCell
				></TableRow
			></TableBody
		></Table
	>
	<Card data-testid="explicit-card" class="gap-0 py-0"
		><p>หนึ่ง</p>
		<p>สอง</p></Card
	>
</main>
