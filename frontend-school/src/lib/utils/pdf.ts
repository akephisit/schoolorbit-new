
import pdfMake from 'pdfmake/build/pdfmake';
// We don't import pdfFonts from build/vfs_fonts because we manage fonts manually
import type { TDocumentDefinitions, CustomTableLayout } from 'pdfmake/interfaces';
import type { TimetableEntry } from '$lib/api/timetable';

// Define fonts
const fonts = {
    Sarabun: {
        normal: window.location.origin + '/fonts/Sarabun-Regular.ttf',
        bold: window.location.origin + '/fonts/Sarabun-Bold.ttf',
        italics: window.location.origin + '/fonts/Sarabun-Regular.ttf', // Fallback
        bolditalics: window.location.origin + '/fonts/Sarabun-Bold.ttf', // Fallback
    }
};

// Helper: Format time
const formatTime = (timeStr: string) => {
    if (!timeStr) return '';
    return timeStr.substring(0, 5);
};

// Helper: Get entry
const getEntry = (entries: TimetableEntry[], day: string, periodId: string) => {
    return entries.find(
        (e) => e.day_of_week === day && e.period_id === periodId && e.is_active
    );
};

// Helper: Define Table Layout
const tableLayout: CustomTableLayout = {
    hLineWidth: (i, node) => (i === 0 || i === node.table.body.length) ? 1 : 1,
    vLineWidth: (i, node) => (i === 0 || i === (node.table.widths?.length ?? 0)) ? 1 : 1,
    hLineColor: (i) => '#9ca3af',
    vLineColor: (i) => '#9ca3af',
    fillColor: (rowIndex, node, columnIndex) => {
        return null;
    }
};

const DAYS = [
    { value: 'MON', label: 'จันทร์', color: '#FEF9C3' },
    { value: 'TUE', label: 'อังคาร', color: '#FCE7F3' },
    { value: 'WED', label: 'พุธ', color: '#DCFCE7' },
    { value: 'THU', label: 'พฤหัสฯ', color: '#FFEDD5' },
    { value: 'FRI', label: 'ศุกร์', color: '#DBEAFE' },
    { value: 'SAT', label: 'เสาร์', color: '#F3F4F6' },
    { value: 'SUN', label: 'อาทิตย์', color: '#F3F4F6' }
];

export const generateTimetablePDF = async (
    title: string,
    subTitle: string,
    periods: any[],
    timetableEntries: TimetableEntry[],
    viewMode: 'CLASSROOM' | 'INSTRUCTOR' = 'CLASSROOM'
) => {
    // 1. Configure Fonts
    pdfMake.fonts = fonts;

    // 2. Build Table Body
    const tableBody: any[][] = [];

    // 2.1 Header Row
    const headerRow = [
        { text: 'วัน / เวลา', bold: true, alignment: 'center', fillColor: '#f3f4f6', margin: [0, 5] }
    ];

    periods.forEach(p => {
        headerRow.push({
            text: [
                { text: `คาบที่ ${p.order_index}\n`, bold: true, fontSize: 10 },
                { text: `${formatTime(p.start_time)} - ${formatTime(p.end_time)}`, fontSize: 8, color: '#4b5563' }
            ],
            alignment: 'center',
            fillColor: '#f3f4f6',
            margin: [0, 2]
        } as any);
    });
    tableBody.push(headerRow);

    // 2.2 Data Rows (MON - FRI)
    DAYS.slice(0, 5).forEach(day => {
        const row: any[] = [];

        // Day Header Column
        row.push({
            text: day.label,
            bold: true,
            alignment: 'center',
            fillColor: day.color,
            fontSize: 12,
            margin: [0, 15] // Try to center vertically approx
        });

        // Period Columns
        periods.forEach(p => {
            const entry = getEntry(timetableEntries, day.value, p.id);
            if (entry) {
                // Build Content Stack
                const stack: any[] = [];

                if (entry.entry_type === 'COURSE') {
                    stack.push(
                        { text: entry.subject_code || '', bold: true, fontSize: 10, color: '#1e3a8a' },
                        { text: entry.subject_name_th || entry.subject_name_en || 'วิชา', fontSize: 9, margin: [0, 0] }
                    );
                } else {
                    // Custom Activity / Break / Homeroom
                    stack.push(
                        { text: entry.title || 'กิจกรรม', bold: true, fontSize: 10, color: '#047857', margin: [0, 2] }
                    );
                }

                // Contextual Info based on View Mode
                if (viewMode === 'CLASSROOM') {
                    // For Student: Show Teacher
                    if (entry.instructor_name) {
                        const teacherName = entry.instructor_name.startsWith('ครู')
                            ? entry.instructor_name
                            : `ครู${entry.instructor_name}`;

                        stack.push({
                            text: teacherName,
                            fontSize: 8,
                            color: '#4b5563',
                            margin: [0, 1]
                        });
                    }
                } else {
                    // For Instructor: Show Class
                    if (entry.classroom_name) {
                        stack.push({
                            text: entry.classroom_name,
                            fontSize: 9,
                            color: '#d97706', // Amber for distinction
                            bold: true,
                            margin: [0, 1]
                        });
                    }
                }

                // Room (Always show if available)
                if (entry.room_code) {
                    stack.push({
                        text: `ห้อง ${entry.room_code}`,
                        fontSize: 8,
                        background: '#f3f4f6',
                        color: '#1f2937',
                        margin: [0, 2],
                        // emulate padding with leading/trailing spaces if needed, but background works on text block
                    });
                } else if (viewMode === 'CLASSROOM' && entry.classroom_name && !entry.instructor_name) {
                    // Fallback for classroom view if absolutely no other info, maybe show class name? 
                    // Actually better to leave blank if no room to avoid confusion "Is M.1/1 a room?"
                }

                row.push({
                    stack: stack,
                    alignment: 'center',
                    margin: [2, 5]
                });
            } else {
                row.push({ text: '' });
            }
        });

        tableBody.push(row);
    });

    // 3. Define Document
    const docDefinition: TDocumentDefinitions = {
        pageSize: 'A4',
        pageOrientation: 'landscape',
        content: [
            { text: title, style: 'header', alignment: 'center', margin: [0, 0, 0, 5] },
            { text: subTitle, style: 'subheader', alignment: 'center', margin: [0, 0, 0, 20] },
            {
                table: {
                    headerRows: 1,
                    widths: ['auto', ...periods.map(() => '*')], // 'auto' for Day, '*' for equal periods
                    body: tableBody
                },
                layout: tableLayout
            },
            {
                columns: [
                    { text: `ข้อมูล ณ วันที่ ${new Date().toLocaleDateString('th-TH')}`, style: 'footer' },
                    { text: 'SchoolOrbit TimeTable', style: 'footer', alignment: 'right' }
                ],
                margin: [0, 10, 0, 0]
            }
        ],
        styles: {
            header: {
                fontSize: 18,
                bold: true,
                color: '#1e3a8a'
            },
            subheader: {
                fontSize: 14,
                color: '#4b5563'
            },
            footer: {
                fontSize: 8,
                color: '#9ca3af'
            }
        },
        defaultStyle: {
            font: 'Sarabun'
        }
    };

    // 4. Download
    pdfMake.createPdf(docDefinition).download(`${title}.pdf`);
};
