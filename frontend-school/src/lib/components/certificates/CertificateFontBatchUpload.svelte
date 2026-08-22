<script lang="ts">
	import { attachCertificateFontBatch, inspectCertificateFontUploads } from '$lib/api/certificates';
	import type {
		AttachSchoolFontBatchRequest,
		InspectSchoolFontUploadsRequest,
		SchoolFontSummary
	} from '$lib/api/school-fonts';
	import SchoolFontBatchUpload from '$lib/components/school-fonts/SchoolFontBatchUpload.svelte';

	let {
		templateId,
		onattached,
		onpendingchange = () => {}
	}: {
		templateId: string;
		onattached: (items: SchoolFontSummary[]) => void;
		onpendingchange?: (pending: boolean) => void;
	} = $props();

	const context = $derived({ type: 'certificate_template' as const, templateId });

	function inspectUploads(payload: InspectSchoolFontUploadsRequest) {
		return inspectCertificateFontUploads(templateId, payload);
	}

	function attachBatch(payload: AttachSchoolFontBatchRequest) {
		return attachCertificateFontBatch(templateId, payload);
	}
</script>

<div class="space-y-4 rounded-xl border bg-muted/15 p-4">
	<div>
		<h4 class="text-sm font-medium">คลังฟอนต์โรงเรียน</h4>
		<p class="mt-1 text-xs leading-relaxed text-muted-foreground">
			ฟอนต์ที่เพิ่มจากหน้าออกแบบนี้จะเข้าคลังกลาง และนำไปใช้กับกิจกรรมอื่นของโรงเรียนได้ทันที
		</p>
	</div>
	<SchoolFontBatchUpload {context} {inspectUploads} {attachBatch} {onattached} {onpendingchange} />
</div>
