/** Extract a full 13-digit ID or application number, including pasted labels and punctuation. */
export function protectedIdentifierSearchCandidate(value: string): string | null {
	const digits = value.match(/\p{Nd}/gu) ?? [];
	return digits.length === 13 && digits.every((digit) => /^[0-9]$/.test(digit))
		? digits.join('')
		: null;
}

/** Fail closed for ambiguous 13+-digit input instead of recording it in URL history. */
export function containsProtectedIdentifier(value: string): boolean {
	return (value.match(/\p{Nd}/gu)?.length ?? 0) >= 13;
}
