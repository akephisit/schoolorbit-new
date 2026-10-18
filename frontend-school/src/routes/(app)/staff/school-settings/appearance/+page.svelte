<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import AppearanceEditor from '#lib/components/appearance/AppearanceEditor.svelte';
	import { getSchoolAppearance, updateSchoolAppearance } from '#lib/appearance/api.js';
	import { applySavedSchoolAppearance } from '#lib/appearance/runtime.js';
	import {
		DEFAULT_APPEARANCE,
		type SchoolAppearance,
		type SchoolAppearanceResponse
	} from '#lib/appearance/model.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';

	let { data }: PageProps = $props();
	const source = $derived(data.appearance);
	const canRead = $derived(
		$authStore.user?.user_type === 'staff' && $can.has(PERMISSIONS.SETTINGS_READ_ALL)
	);
	const canUpdate = $derived(canRead && $can.has(PERMISSIONS.SETTINGS_UPDATE_ALL));
	let draft = $state<SchoolAppearance>({ ...DEFAULT_APPEARANCE });
	let saved = $state<SchoolAppearanceResponse | null>(null);
	let loading = $state(true),
		saving = $state(false),
		loadError = $state(''),
		saveError = $state(''),
		notice = $state(''),
		conflict = $state(false),
		fontRetry = $state(false);
	const request = new LatestRequest();
	let owner = '',
		epoch = 0,
		disposed = false;
	let mutation: AbortController | undefined;
	function applyLoaded(result: Awaited<typeof data.appearance>, revision: number) {
		if (!request.isCurrent(revision) || !canRead) return;
		loading = false;
		if (!result.ok) {
			loadError = result.error;
			return;
		}
		if (result.data.identityKey !== appIdentityKey()) return;
		saved = result.data.settings;
		if (saved) draft = { ...saved.appearance };
		conflict = false;
		saveError = '';
		notice = '';
		fontRetry = false;
	}
	$effect.pre(() => {
		const operation = source,
			identity = `${$authStore.user?.id ?? ''}|${canRead}|${canUpdate}`;
		untrack(() => {
			if (owner !== identity) {
				owner = identity;
				epoch++;
				mutation?.abort();
				saving = false;
				saved = null;
				draft = { ...DEFAULT_APPEARANCE };
				saveError = '';
				notice = '';
				conflict = false;
				fontRetry = false;
			}
			const ticket = request.begin();
			loading = true;
			loadError = '';
			void operation.then((result) => applyLoaded(result, ticket.revision));
		});
		return () => request.abort();
	});
	onDestroy(() => {
		disposed = true;
		epoch++;
		mutation?.abort();
		request.abort();
	});
	async function reload() {
		if (!canRead || saving || disposed) return;
		const ticket = request.begin();
		loading = true;
		loadError = '';
		const identityKey = appIdentityKey();
		const result = await captureRouteLoad(
			getSchoolAppearance({ signal: ticket.signal }).then((settings) => ({
				identityKey,
				settings
			})),
			'โหลดรูปแบบโรงเรียนไม่สำเร็จ'
		);
		applyLoaded(result, ticket.revision);
	}
	async function save() {
		if (!canUpdate || !saved || saving || conflict || disposed) return;
		const mutationEpoch = epoch,
			identity = appIdentityKey();
		const current = () =>
			!disposed && epoch === mutationEpoch && identity === appIdentityKey() && canUpdate;
		mutation?.abort();
		mutation = new AbortController();
		const signal = mutation.signal;
		const body = { appearance: { ...draft }, expectedRevision: saved.revision };
		request.abort();
		loading = false;
		saving = true;
		saveError = '';
		notice = '';
		fontRetry = false;
		try {
			const result = await updateSchoolAppearance(body, { signal });
			if (!current()) return;
			saved = result;
			draft = { ...result.appearance };
			// The database save has completed. A font failure is a presentation retry, never a failed save.
			const applied = await applySavedSchoolAppearance(result);
			if (!current()) return;
			fontRetry = applied === 'font-failed';
			notice = fontRetry
				? 'บันทึกรูปแบบของทั้งโรงเรียนแล้ว แต่โหลดฟอนต์ไม่สำเร็จ หน้านี้ยังใช้รูปแบบเดิมชั่วคราว'
				: 'บันทึกรูปแบบของทั้งโรงเรียนแล้ว';
		} catch (error) {
			if (!current()) return;
			conflict = error instanceof ApiClientError && error.status === 409;
			saveError = conflict
				? 'มีผู้เปลี่ยนรูปแบบโรงเรียนระหว่างที่คุณแก้ไข แบบร่างของคุณยังอยู่ กรุณาโหลดค่าล่าสุดและตรวจทานก่อนบันทึกอีกครั้ง'
				: error instanceof Error
					? error.message
					: 'บันทึกไม่สำเร็จ แบบร่างของคุณยังอยู่';
		} finally {
			if (current()) saving = false;
		}
	}
	async function retryFont() {
		if (!saved || saving || !canRead || disposed) return;
		const mutationEpoch = epoch;
		saving = true;
		const result = await applySavedSchoolAppearance(saved);
		if (disposed || mutationEpoch !== epoch) return;
		fontRetry = result === 'font-failed';
		saving = false;
		notice = fontRetry
			? 'บันทึกแล้ว แต่ยังโหลดฟอนต์ไม่สำเร็จ กรุณาลองอีกครั้ง'
			: 'บันทึกและแสดงรูปแบบล่าสุดแล้ว';
	}
</script>

<PageShell
	title="รูปแบบและตัวอักษร"
	description="ตั้งค่าสี ฟอนต์ และระยะห่างสำหรับบุคลากร นักเรียน ผู้ปกครอง และหน้าสาธารณะของทั้งโรงเรียน"
	backHref="/staff/school-settings"
>
	<section data-testid="school-appearance-settings" aria-busy={loading || saving}>
		{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์ดูรูปแบบโรงเรียน" />
		{:else if loadError}<PageState
				variant="error"
				title="โหลดรูปแบบโรงเรียนไม่สำเร็จ"
				description={loadError}
				actionLabel="ลองอีกครั้ง"
				onaction={reload}
			/>
		{:else if loading && !saved}<PageSkeleton variant="form" rows={3} />{/if}
		{#if canRead && saved}
			<div class="mb-4 space-y-2" aria-live="polite">
				<p class="text-sm text-muted-foreground">
					ใช้กับทั้งโรงเรียน · โหมดสว่าง/มืดของแต่ละคนยังเลือกได้ตามเดิม ·
					การคืนค่าเริ่มต้นจะยังไม่บันทึกจนกดบันทึก
				</p>
				{#if !canUpdate}<p class="text-sm text-muted-foreground">
						คุณดูรูปแบบได้ แต่ไม่มีสิทธิ์บันทึกการเปลี่ยนแปลง
					</p>{/if}
				{#if saveError}<p role="alert" class="text-destructive">{saveError}</p>{/if}
				{#if conflict}<Button variant="outline" onclick={reload} disabled={saving || loading}
						>โหลดค่าล่าสุดเพื่อตรวจทาน (แทนแบบร่าง)</Button
					>{/if}
				{#if notice}<p role="status">{notice}</p>{/if}
				{#if fontRetry}<Button variant="outline" onclick={retryFont} disabled={saving}
						>ลองโหลดฟอนต์อีกครั้ง</Button
					>{/if}
			</div>
			<AppearanceEditor
				bind:draft
				readonly={!canUpdate}
				disabled={loading || conflict}
				{saving}
				onsave={save}
			/>
		{/if}
	</section>
</PageShell>
