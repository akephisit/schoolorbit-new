export const MAX_CERTIFICATE_BATCH_SIZE = 200;

function isUnsafeFilenameControl(character: string): boolean {
	const codePoint = character.codePointAt(0) ?? 0;
	return (
		codePoint <= 0x1f ||
		(codePoint >= 0x7f && codePoint <= 0x9f) ||
		(codePoint >= 0x202a && codePoint <= 0x202e) ||
		(codePoint >= 0x2066 && codePoint <= 0x2069)
	);
}

export function validateCertificateBatchSize(count: number): void {
	if (!Number.isSafeInteger(count) || count < 1) {
		throw new Error('ต้องเลือกเกียรติบัตรอย่างน้อย 1 ใบ');
	}
	if (count > MAX_CERTIFICATE_BATCH_SIZE) {
		throw new Error(`สร้าง PDF ได้ครั้งละไม่เกิน ${MAX_CERTIFICATE_BATCH_SIZE} ใบ`);
	}
}

export function sanitizeCertificateFilename(value: string): string {
	let base = value
		.normalize('NFC')
		.trim()
		.replace(/\.pdf$/iu, '');
	base = base
		.split('')
		.filter((character) => !isUnsafeFilenameControl(character))
		.join('')
		.replace(/[\\/:*?"<>|]+/gu, '-')
		.replace(/\s+/gu, ' ')
		.replace(/-+/gu, '-')
		.replace(/^[.\s-]+|[.\s-]+$/gu, '');
	base = Array.from(base)
		.slice(0, 120)
		.join('')
		.replace(/[.\s-]+$/gu, '');
	if (!base) base = 'เกียรติบัตร';
	return `${base}.pdf`;
}

export function downloadCertificatePdf(bytes: Uint8Array, suggestedFilename: string): void {
	const blob = new Blob([bytes as BlobPart], { type: 'application/pdf' });
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = sanitizeCertificateFilename(suggestedFilename);
	document.body.appendChild(link);
	link.click();
	link.remove();
	window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
}
