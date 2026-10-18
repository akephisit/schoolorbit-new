/** Match the server's Unicode scalar bounds after the title/sender payload trim. */
export function validateDocumentMetadata(input: {
	title: string;
	sender: string;
	documentNumber?: string | null;
}): string | null {
	if (!input.title.trim() || !input.sender.trim()) return 'กรอกเรื่องและผู้ส่งให้ครบ';
	if ([...input.title.trim()].length > 300) return 'เรื่องต้องไม่เกิน 300 ตัวอักษร';
	if ([...input.sender.trim()].length > 300) return 'ผู้ส่งต้องไม่เกิน 300 ตัวอักษร';
	if ([...(input.documentNumber ?? '')].length > 150)
		return 'เลขที่หนังสือต้นทางต้องไม่เกิน 150 ตัวอักษร';
	return null;
}
