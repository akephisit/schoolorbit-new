<script lang="ts">
	import { browser } from '$app/env';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { authStore } from '#lib/stores/auth.js';
	import {
		createCertificateTemplatePreviewManifest,
		getCertificateTemplate,
		getCertificateTemplateVariableCatalog,
		listCertificateSchoolFonts,
		type CertificateTemplateDetail,
		type CertificateRenderManifest
	} from '#lib/api/certificates.js';
	import type { SchoolFontSummary } from '#lib/api/school-fonts.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import CertificateEditor from '#lib/components/certificates/editor/CertificateEditor.svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	let { data }: PageProps = $props();
	const campaignId = $derived(page.params.campaignId ?? ''),
		templateId = $derived(page.params.templateId ?? '');
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const context = $derived(`${identityKey}|${campaignId}|${templateId}`);
	const canRead = $derived(
		$can.hasAny(PERMISSIONS.CERTIFICATE_READ_ORGANIZATION_UNIT, PERMISSIONS.CERTIFICATE_READ_SCHOOL)
	);
	const canUpdate = $derived(
		$can.hasAny(
			PERMISSIONS.CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
			PERMISSIONS.CERTIFICATE_UPDATE_SCHOOL
		)
	);
	let template = $state.raw<CertificateTemplateDetail | null>(null),
		manifest = $state.raw<CertificateRenderManifest | null>(null),
		variables = $state.raw<string[]>([]),
		schoolFonts = $state.raw<SchoolFontSummary[]>([]);
	let loading = $state(true),
		error = $state(''),
		manifestLoading = $state(false),
		manifestError = $state(''),
		variablesLoading = $state(false),
		variablesError = $state(''),
		fontsLoading = $state(false),
		fontsError = $state('');
	const primaryRequest = new LatestRequest(),
		manifestRequest = new LatestRequest(),
		variablesRequest = new LatestRequest(),
		fontsRequest = new LatestRequest();
	let owner = '',
		disposed = false,
		openedOwner = '';
	let consumed: typeof data.template | null = null;
	$effect.pre(() => {
		const key = context,
			source = data.template,
			allowed = canRead;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				primaryRequest.abort();
				manifestRequest.abort();
				variablesRequest.abort();
				fontsRequest.abort();
				openedOwner = '';
				template = null;
				manifest = null;
				variables = [];
				schoolFonts = [];
				loading = allowed;
				error = '';
				manifestError = '';
				variablesError = '';
				fontsError = '';
				manifestLoading = false;
				variablesLoading = false;
				fontsLoading = false;
			}
			if (!allowed || consumed === source) return;
			consumed = source;
			const t = primaryRequest.begin();
			loading = true;
			void source.then((v) => applyPrimary(v, t.revision));
		});
	});
	$effect(() => {
		const key = context,
			allowed = canUpdate && template?.capabilities.canUpdate === true;
		untrack(() => {
			if (!browser || !allowed || disposed || openedOwner === key) return;
			openedOwner = key;
			void retryManifest();
			void retryVariables();
			void retryFonts();
		});
	});
	onDestroy(() => {
		disposed = true;
		primaryRequest.abort();
		manifestRequest.abort();
		variablesRequest.abort();
		fontsRequest.abort();
	});
	function applyPrimary(v: Awaited<typeof data.template>, revision: number) {
		if (!primaryRequest.isCurrent(revision)) return;
		loading = false;
		if (!v.ok) {
			error = v.error;
			return;
		}
		if (v.data.ownerKey !== context) return;
		template = v.data.record;
		error = '';
	}
	async function retryTemplate() {
		if (disposed || !canRead) return;
		const ownerKey = context,
			campaign = campaignId,
			t = primaryRequest.begin();
		loading = true;
		error = '';
		applyPrimary(
			await captureRouteLoad(
				getCertificateTemplate(templateId, { signal: t.signal }).then((record) => {
					if (record.campaignId !== campaign)
						throw new Error('แบบเกียรติบัตรนี้ไม่ได้อยู่ในกิจกรรมตาม URL');
					return { ownerKey, record };
				}),
				'โหลดแบบเกียรติบัตรไม่สำเร็จ'
			),
			t.revision
		);
	}
	function current(key: string) {
		return !disposed && key === context && canUpdate && template?.capabilities.canUpdate === true;
	}
	async function retryManifest() {
		const key = context;
		if (!current(key)) return;
		const t = manifestRequest.begin();
		manifestLoading = true;
		manifestError = '';
		const v = await captureRouteLoad(
			createCertificateTemplatePreviewManifest(
				templateId,
				{ previewKind: 'short' },
				{ signal: t.signal }
			),
			'โหลดพื้นหลังสำหรับ editor ไม่สำเร็จ'
		);
		if (!manifestRequest.isCurrent(t.revision) || !current(key)) return;
		manifestLoading = false;
		if (v.ok) manifest = v.data;
		else manifestError = v.error;
	}
	async function retryVariables() {
		const key = context;
		if (!current(key)) return;
		const t = variablesRequest.begin();
		variablesLoading = true;
		variablesError = '';
		const v = await captureRouteLoad(
			getCertificateTemplateVariableCatalog(templateId, { signal: t.signal }),
			'โหลดตัวแปรไม่สำเร็จ'
		);
		if (!variablesRequest.isCurrent(t.revision) || !current(key)) return;
		variablesLoading = false;
		if (v.ok) variables = v.data.variables;
		else variablesError = v.error;
	}
	async function retryFonts() {
		const key = context;
		if (!current(key)) return;
		const t = fontsRequest.begin();
		fontsLoading = true;
		fontsError = '';
		const v = await captureRouteLoad(
			listCertificateSchoolFonts(templateId, { signal: t.signal }),
			'โหลดคลังฟอนต์ไม่สำเร็จ'
		);
		if (!fontsRequest.isCurrent(t.revision) || !current(key)) return;
		fontsLoading = false;
		if (v.ok) schoolFonts = v.data.items;
		else fontsError = v.error;
	}
</script>

<PageShell
	title={template ? `ออกแบบ · ${template.name}` : 'ออกแบบเกียรติบัตร'}
	description="วางข้อความ ตัวแปร รูปภาพ และ QR Code บน PDF พื้นหลัง แล้วตรวจด้วย renderer เดียวกับไฟล์จริง"
	backHref={resolve(`staff/certificates/${campaignId}/templates`)}
	backLabel="กลับไปแบบเกียรติบัตร"
	contentClass="space-y-0 pb-3"
>
	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์เปิด editor"
			description="สิทธิ์อ่านและหน่วยงานเจ้าของกิจกรรมตรวจจาก backend"
		/>
	{:else}
		{#if loading && !template}<div role="status" aria-label="กำลังโหลดแบบสำหรับ editor">
				<PageSkeleton variant="detail" />
			</div>{/if}
		{#if error}<PageState
				variant="error"
				title="เปิด editor ไม่สำเร็จ"
				description={error}
				actionLabel="ลองแบบอีกครั้ง"
				onaction={retryTemplate}
			/>{/if}
		{#if template}
			{#if !canUpdate || !template.capabilities.canUpdate}
				<PageState
					variant="permission"
					title="ไม่มีสิทธิ์แก้แบบนี้"
					description="คุณยังดูข้อมูลกิจกรรมได้ แต่ไม่มีสิทธิ์แก้ layout ของแบบนี้"
				/>
			{:else}
				{#if variablesLoading}<p role="status">กำลังโหลดตัวแปร</p>{/if}
				{#if variablesError}<PageState
						variant="error"
						title="โหลดตัวแปรไม่สำเร็จ"
						description={variablesError}
						actionLabel="ลองตัวแปรอีกครั้ง"
						onaction={retryVariables}
					/>{/if}
				{#if fontsLoading}<p role="status">กำลังโหลดฟอนต์</p>{/if}
				{#if fontsError}<PageState
						variant="error"
						title="โหลดฟอนต์ไม่สำเร็จ"
						description={fontsError}
						actionLabel="ลองฟอนต์อีกครั้ง"
						onaction={retryFonts}
					/>{/if}
				{#if manifestLoading && !manifest}<div
						role="status"
						aria-label="กำลังโหลดพื้นหลังสำหรับ editor"
					>
						<PageSkeleton variant="detail" />
					</div>{/if}
				{#if manifestError}<PageState
						variant="error"
						title="โหลดพื้นหลังไม่สำเร็จ"
						description={manifestError}
						actionLabel="ลองพื้นหลังอีกครั้ง"
						onaction={retryManifest}
					/>{/if}
				{#if manifest}
					{#key context}<CertificateEditor
							{template}
							initialManifest={manifest}
							{variables}
							{schoolFonts}
						/>{/key}
				{/if}
			{/if}
		{/if}
	{/if}
</PageShell>
