<script lang="ts">
	import { toast } from 'svelte-sonner';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Textarea } from '$lib/components/ui/textarea';
	import { buildCalendarEmbedCode, buildCalendarEmbedUrl } from '$lib/utils/calendar';
	import { Copy } from 'lucide-svelte';

	let {
		open = $bindable(false),
		origin
	}: {
		open: boolean;
		origin: string;
	} = $props();

	const embedUrl = $derived(buildCalendarEmbedUrl(origin));
	const embedCode = $derived(buildCalendarEmbedCode(origin));

	async function copyEmbedCode() {
		try {
			await navigator.clipboard.writeText(embedCode);
			toast.success('คัดลอกโค้ดแล้ว');
		} catch {
			toast.error('คัดลอกไม่สำเร็จ เลือกและคัดลอกโค้ดด้านล่างด้วยตนเอง');
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header>
			<Dialog.Title>ฝังปฏิทินในเว็บไซต์</Dialog.Title>
			<Dialog.Description>
				เพิ่มบล็อก Custom HTML ใน WordPress แล้ววางโค้ด ปฏิทินจะอัปเดตตามข้อมูลสาธารณะ
				ใน SchoolOrbit โดยอัตโนมัติ
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<ol class="grid gap-2 text-sm sm:grid-cols-2">
				<li class="flex items-center gap-2 rounded-lg border bg-muted/30 px-3 py-2.5">
					<span
						class="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-foreground"
						>1</span
					>
					เพิ่มบล็อก Custom HTML ในหน้าปฏิทินของ WordPress
				</li>
				<li class="flex items-center gap-2 rounded-lg border bg-muted/30 px-3 py-2.5">
					<span
						class="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-foreground"
						>2</span
					>
					คัดลอกโค้ดด้านล่างไปวาง แล้วเผยแพร่หน้าเว็บไซต์
				</li>
			</ol>

			<section aria-labelledby="calendar-embed-preview-title">
				<div class="overflow-hidden rounded-xl border bg-background shadow-sm">
					<div
						class="flex items-center justify-between gap-3 border-b bg-muted/40 px-3 py-2 text-xs text-muted-foreground"
					>
						<div class="flex items-center gap-1.5" aria-hidden="true">
							<span class="size-2 rounded-full bg-red-400"></span>
							<span class="size-2 rounded-full bg-amber-400"></span>
							<span class="size-2 rounded-full bg-emerald-400"></span>
						</div>
						<span id="calendar-embed-preview-title" class="font-medium text-foreground">
							ตัวอย่างบนเว็บไซต์
						</span>
						<span class="hidden max-w-64 truncate font-mono sm:block">{embedUrl}</span>
					</div>
					<iframe
						src={embedUrl}
						title="ตัวอย่างปฏิทินโรงเรียน"
						class="h-[28rem] w-full border-0"
						loading="lazy"
						sandbox="allow-scripts allow-same-origin"
						referrerpolicy="strict-origin-when-cross-origin"
					></iframe>
				</div>
			</section>

			<div class="grid gap-2">
				<label for="calendar-embed-code" class="text-sm font-medium">โค้ดสำหรับ WordPress</label>
				<Textarea
					id="calendar-embed-code"
					value={embedCode}
					readonly
					rows={9}
					class="font-mono text-xs leading-relaxed"
					onfocus={(event) => event.currentTarget.select()}
				/>
				<p class="text-xs text-muted-foreground">
					โค้ดนี้แสดงเฉพาะกิจกรรมที่ตั้งสถานะเป็นสาธารณะ
				</p>
			</div>
		</div>

		<Dialog.Footer>
			<Button type="button" variant="outline" onclick={() => (open = false)}>ปิด</Button>
			<Button type="button" onclick={copyEmbedCode}>
				<Copy class="size-4" />
				คัดลอกโค้ด
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
