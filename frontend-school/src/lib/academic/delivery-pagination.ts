export function deliveryChangePage<T>(items: readonly T[], page: number, size = 8) {
	const pageCount = Math.max(1, Math.ceil(items.length / size));
	const currentPage = Math.max(0, Math.min(page, pageCount - 1));
	return {
		items: items.slice(currentPage * size, (currentPage + 1) * size),
		page: currentPage,
		pageCount,
		total: items.length
	};
}
