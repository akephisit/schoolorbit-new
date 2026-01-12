import type { ZodType, ZodError } from 'zod';

/**
 * Validation error result
 */
export interface ValidationError {
	field: string;
	message: string;
}

/**
 * Validation result
 */
export interface ValidationResult<T> {
	success: boolean;
	data?: T;
	errors?: ValidationError[];
}

/**
 * Validate data against a Zod schema
 */
export function validate<T>(schema: ZodType<T>, data: unknown): ValidationResult<T> {
	const result = schema.safeParse(data);

	if (result.success) {
		return {
			success: true,
			data: result.data
		};
	}

	return {
		success: false,
		errors: formatZodErrors(result.error)
	};
}

/**
 * Format Zod errors into a more usable format
 */
export function formatZodErrors(error: ZodError): ValidationError[] {
	return error.issues.map((err) => ({
		field: err.path.join('.'),
		message: err.message
	}));
}

/**
 * Get error message for a specific field
 */
export function getFieldError(
	errors: ValidationError[] | undefined,
	field: string
): string | undefined {
	if (!errors) return undefined;
	return errors.find((err: ValidationError) => err.field === field)?.message;
}

/**
 * Check if a field has an error
 */
export function hasFieldError(errors: ValidationError[] | undefined, field: string): boolean {
	if (!errors) return false;
	return errors.some((err) => err.field === field);
}

/**
 * Get all error messages as an array
 */
export function getAllErrorMessages(errors: ValidationError[] | undefined): string[] {
	if (!errors) return [];
	return errors.map((err) => err.message);
}

/**
 * Validate a single field
 */
export function validateField<T>(
	schema: ZodType<T>,
	data: unknown,
	field: string
): string | undefined {
	const result = schema.safeParse(data);

	if (result.success) {
		return undefined;
	}

	const fieldError = result.error.issues.find((err) => err.path.join('.') === field);
	return fieldError?.message;
}
