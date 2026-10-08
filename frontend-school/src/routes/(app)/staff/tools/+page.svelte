<script lang="ts">
	import { resolve } from '$app/paths';
	import { QrCode, ArrowRight, Wrench } from '@lucide/svelte';
	import PageShell from '#lib/components/app-layout/PageShell.svelte';
</script>

<PageShell
	title="เครื่องมือ"
	description="เครื่องมือสำหรับงานประจำวันของครูและบุคลากร"
	icon={Wrench}
>
	<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
		<a
			href={resolve('staff/tools/qr-code')}
			class="group rounded-xl border border-border bg-card p-6 text-card-foreground transition-colors hover:border-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
		>
			<QrCode class="mb-4 size-8 text-primary" />
			<h2 class="text-lg font-semibold">สร้าง QR Code</h2>
			<p class="mt-2 text-sm text-muted-foreground">
				เปลี่ยนลิงก์หรือข้อความเป็น QR Code พร้อมใส่โลโก้และดาวน์โหลดเป็นภาพ PNG
			</p>
			<span class="mt-4 inline-flex items-center gap-2 text-sm font-medium text-primary">
				เปิดเครื่องมือ <ArrowRight class="size-4" />
			</span>
		</a>
	</div>
</PageShell>
