<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		listSignatureAssets,
		createSignatureAsset,
		type SignatureAsset
	} from '#lib/api/correspondence.js';
	import { uploadFile } from '#lib/api/files.js';
	import { authStore } from '#lib/stores/auth.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { CorrespondenceRegion } from '#lib/correspondence/region.svelte.js';
	import { hasInk, type InkPath } from '#lib/correspondence/geometry.js';
	import { dateLabel } from '#lib/correspondence/presentation.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import SignaturePad from './SignaturePad.svelte';
	let { source }: { source: Promise<RouteLoadResult<SignatureAsset[]>> } = $props();
	const region = new CorrespondenceRegion<SignatureAsset[]>();
	let label = $state(''),
		paths = $state<InkPath[]>([]),
		image = $state<File | null>(null),
		imageUrl = $state(''),
		pending = $state(false),
		error = $state(''),
		adding = $state(false);
	let disposed = false;
	$effect.pre(() => {
		const operation = source;
		untrack(() => region.consume(operation));
	});
	$effect(() => {
		if (!image) {
			imageUrl = '';
			return;
		}
		const url = URL.createObjectURL(image);
		imageUrl = url;
		return () => URL.revokeObjectURL(url);
	});
	onDestroy(() => {
		disposed = true;
		region.dispose();
	});
	async function drawnImage(): Promise<File> {
		const canvas = document.createElement('canvas');
		canvas.width = 1000;
		canvas.height = 360;
		const context = canvas.getContext('2d');
		if (!context) throw new Error('สร้างภาพลายมือชื่อไม่สำเร็จ');
		context.strokeStyle = '#111827';
		context.lineWidth = 3;
		context.lineCap = 'round';
		context.lineJoin = 'round';
		for (const path of paths) {
			context.beginPath();
			path.points.forEach((point, index) => {
				if (index) context.lineTo(point.x * 1000, point.y * 360);
				else context.moveTo(point.x * 1000, point.y * 360);
			});
			context.stroke();
		}
		const blob = await new Promise<Blob>((resolve, reject) =>
			canvas.toBlob(
				(result) => (result ? resolve(result) : reject(new Error('สร้างภาพลายมือชื่อไม่สำเร็จ'))),
				'image/png'
			)
		);
		return new File([blob], 'signature.png', { type: 'image/png' });
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !$authStore.user) return;
		error = '';
		if (!label.trim()) {
			error = 'ระบุชื่อลายมือชื่อ';
			return;
		}
		if (!image && !hasInk(paths)) {
			error = 'วาดลายมือชื่อหรือเลือกภาพ';
			return;
		}
		pending = true;
		try {
			const file = image ?? (await drawnImage());
			const uploaded = await uploadFile(file, 'correspondence_signature', $authStore.user.id);
			if (disposed) return;
			const asset = await createSignatureAsset(label.trim(), uploaded.id);
			if (disposed) return;
			region.patch([asset, ...(region.value ?? [])]);
			adding = false;
			label = '';
			paths = [];
			image = null;
		} catch (cause) {
			if (!disposed) error = cause instanceof Error ? cause.message : 'บันทึกลายมือชื่อไม่สำเร็จ';
		} finally {
			if (!disposed) pending = false;
		}
	}
</script>

<PageShell
	title="ลายมือชื่อของฉัน"
	backHref="/staff/correspondence"
	description="เก็บลายมือชื่อส่วนตัวไว้เลือกใช้ คุณต้องยืนยันทุกครั้งที่ลงนามหนังสือ"
>
	{#snippet actions()}<Button
			onclick={() => {
				adding = true;
			}}>เพิ่มลายมือชื่อ</Button
		>{/snippet}
	{#if adding}<Card.Root class="max-w-2xl"
			><Card.Header
				><Card.Title>เพิ่มลายมือชื่อส่วนตัว</Card.Title><Card.Description
					>ภาพที่บันทึกใหม่ไม่เปลี่ยนหลักฐานในหนังสือที่ลงนามแล้ว</Card.Description
				></Card.Header
			><Card.Content
				><form class="space-y-4" onsubmit={save}>
					{#if error}<PageState variant="error" title={error} />{/if}
					<div class="space-y-2">
						<Label for="signature-label">ชื่อลายมือชื่อ *</Label><Input
							id="signature-label"
							bind:value={label}
							disabled={pending}
							required
						/>
					</div>
					<SignaturePad
						bind:paths
						disabled={pending}
						onimage={(file) => {
							image = file;
						}}
					/>{#if imageUrl}<div class="space-y-2 rounded-md border bg-white p-3">
							<img
								src={imageUrl}
								alt="ภาพลายมือชื่อก่อนบันทึก"
								class="h-28 w-full object-contain"
							/><Button
								type="button"
								variant="outline"
								disabled={pending}
								onclick={() => {
									image = null;
								}}>ใช้ลายมือชื่อที่วาดแทน</Button
							>
						</div>{/if}
					<div class="flex justify-end gap-2">
						<Button
							type="button"
							variant="outline"
							disabled={pending}
							onclick={() => {
								adding = false;
							}}>ยกเลิก</Button
						><LoadingButton type="submit" loading={pending}>บันทึกลายมือชื่อ</LoadingButton>
					</div>
				</form></Card.Content
			></Card.Root
		>{/if}
	<section data-testid="signature-library" aria-busy={region.loading}>
		{#if region.error}<PageState
				variant="error"
				title={region.error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => region.retry((signal) => listSignatureAssets({ signal }))}
			/>{/if}{#if region.loading && !region.value}<PageSkeleton
			/>{:else if region.value?.length}<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
				{#each region.value as asset (asset.id)}<Card.Root
						><Card.Header
							><Card.Title>{asset.label}</Card.Title><Card.Description
								>{dateLabel(asset.createdAt)}</Card.Description
							></Card.Header
						><Card.Content
							><div class="rounded-md border bg-white p-3">
								<PrivateFileImage
									fileId={asset.fileId}
									resourceId={$authStore.user?.id}
									alt={`ลายมือชื่อ ${asset.label}`}
									class="h-28 w-full object-contain"
								/>
							</div></Card.Content
						></Card.Root
					>{/each}
			</div>{:else if !region.error}<PageState
				title="ยังไม่มีลายมือชื่อที่บันทึก"
				description="คุณวาดลายมือชื่อใหม่ขณะลงนามได้ หรือเพิ่มภาพส่วนตัวไว้ใช้ซ้ำ"
			/>{/if}
	</section>
</PageShell>
