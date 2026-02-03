export function formatDate(dateStr: string | Date | undefined | null): string {
    if (!dateStr) return '-';

    const date = typeof dateStr === 'string' ? new Date(dateStr) : dateStr;

    if (isNaN(date.getTime())) return '-';

    return new Intl.DateTimeFormat('th-TH', {
        year: 'numeric',
        month: 'long',
        day: 'numeric'
    }).format(date);
}

export function formatDateTime(dateStr: string | Date | undefined | null): string {
    if (!dateStr) return '-';

    const date = typeof dateStr === 'string' ? new Date(dateStr) : dateStr;

    if (isNaN(date.getTime())) return '-';

    return new Intl.DateTimeFormat('th-TH', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
    }).format(date);
}
