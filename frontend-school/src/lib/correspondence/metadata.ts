export function documentMetadataErrors(input: {
	title: string;
	sender: string;
	documentNumber?: string | null;
}): Partial<Record<'title' | 'sender' | 'documentNumber', string>> {
	const errors: Partial<Record<'title' | 'sender' | 'documentNumber', string>> = {};
	if (!input.title.trim()) errors.title = 'กรอกเรื่อง';
	else if ([...input.title.trim()].length > 300) errors.title = 'เรื่องต้องไม่เกิน 300 ตัวอักษร';
	if (!input.sender.trim()) errors.sender = 'กรอกผู้ส่ง / หน่วยงานต้นทาง';
	else if ([...input.sender.trim()].length > 300) errors.sender = 'ผู้ส่งต้องไม่เกิน 300 ตัวอักษร';
	if ([...(input.documentNumber ?? '')].length > 150)
		errors.documentNumber = 'เลขที่หนังสือต้นทางต้องไม่เกิน 150 ตัวอักษร';
	return errors;
}

/** Match the server's Unicode scalar bounds after the title/sender payload trim. */
export function validateDocumentMetadata(
	input: Parameters<typeof documentMetadataErrors>[0]
): string | null {
	return Object.values(documentMetadataErrors(input))[0] ?? null;
}
