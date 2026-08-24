import assert from 'node:assert/strict';
import test from 'node:test';

test('builds one instructor PDF page from the loaded self-service timetable', async () => {
	const module = await import('../../src/lib/utils/staff-own-timetable-pdf.ts').catch(() => ({}));
	assert.equal(typeof module.buildStaffOwnTimetablePdfDownload, 'function');

	const entries = [{ id: 'entry-1', room_code: 'MATH-1' }];
	const result = module.buildStaffOwnTimetablePdfDownload({
		teacherName: 'สายใจ / วิทยา',
		termName: '',
		termCode: '1',
		academicYearName: 'ปีการศึกษา 2569',
		entries,
		dayValues: ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT'],
		periods: [
			{
				id: 'period-2',
				name: 'คาบ 2',
				startTime: '09:20:00',
				endTime: '10:10:00'
			},
			{ id: 'period-activity', name: 'กิจกรรม' }
		]
	});

	assert.equal(result.fileName, 'ตารางสอน ครูสายใจ - วิทยา ภาคเรียนที่ 1 ปีการศึกษา 2569');
	assert.deepEqual(result.pages, [
		{
			title: 'ตารางสอน ครูสายใจ / วิทยา',
			subTitle: 'ภาคเรียนที่ 1 ปีการศึกษา 2569',
			dayValues: ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT'],
			periods: [
				{
					id: 'period-2',
					order_index: 0,
					name: 'คาบ 2',
					start_time: '09:20:00',
					end_time: '10:10:00'
				},
				{
					id: 'period-activity',
					order_index: 1,
					name: 'กิจกรรม',
					start_time: '',
					end_time: ''
				}
			],
			timetableEntries: entries,
			viewMode: 'INSTRUCTOR'
		}
	]);
});

test('allows export only for loaded data matching the current valid selection', async () => {
	const module = await import('../../src/lib/utils/staff-own-timetable-pdf.ts');
	assert.equal(typeof module.canDownloadStaffOwnTimetablePdf, 'function');

	const readyState = {
		loading: false,
		isExporting: false,
		selectedYearId: 'year-2569',
		selectedAcademicTermId: 'term-1',
		selectedTermYearId: 'year-2569',
		loadedSelectionKey: 'year-2569:term-1',
		entryCount: 5,
		periodCount: 8
	};

	assert.equal(module.canDownloadStaffOwnTimetablePdf(readyState), true);
	assert.equal(
		module.canDownloadStaffOwnTimetablePdf({
			...readyState,
			loadedSelectionKey: 'year-2568:term-2'
		}),
		false
	);
	assert.equal(
		module.canDownloadStaffOwnTimetablePdf({
			...readyState,
			selectedTermYearId: 'year-2568'
		}),
		false
	);
	assert.equal(module.canDownloadStaffOwnTimetablePdf({ ...readyState, loading: true }), false);
	assert.equal(module.canDownloadStaffOwnTimetablePdf({ ...readyState, isExporting: true }), false);
	assert.equal(module.canDownloadStaffOwnTimetablePdf({ ...readyState, entryCount: 0 }), false);
	assert.equal(module.canDownloadStaffOwnTimetablePdf({ ...readyState, periodCount: 0 }), false);
});

test('resolves configured PDF days in school-week order with a weekday fallback', async () => {
	const module = await import('../../src/lib/utils/timetable-pdf-days.ts').catch(() => ({}));
	assert.equal(typeof module.resolveTimetablePdfDayValues, 'function');

	assert.deepEqual(module.resolveTimetablePdfDayValues(['SUN', 'MON', 'SAT', 'UNKNOWN', 'MON']), [
		'MON',
		'SAT',
		'SUN'
	]);
	assert.deepEqual(module.resolveTimetablePdfDayValues([]), ['MON', 'TUE', 'WED', 'THU', 'FRI']);
});

test('runs the full-layout download workflow and always clears exporting state', async () => {
	const module = await import('../../src/lib/utils/staff-own-timetable-pdf.ts');
	assert.equal(typeof module.runStaffOwnTimetablePdfDownload, 'function');

	const download = { pages: [{ title: 'ตารางสอน ครูสายใจ' }], fileName: 'ตารางสอน ครูสายใจ' };
	const exportingStates = [];
	const generatorCalls = [];
	const errors = [];
	let successCount = 0;
	const failure = new Error('pdf failed');

	await module.runStaffOwnTimetablePdfDownload(download, {
		generatePdf: async (...args) => {
			generatorCalls.push(args);
			throw failure;
		},
		setExporting: (value) => exportingStates.push(value),
		onSuccess: () => {
			successCount += 1;
		},
		onError: (error) => errors.push(error)
	});

	assert.deepEqual(generatorCalls, [[download.pages, download.fileName, { layout: 'full' }]]);
	assert.deepEqual(exportingStates, [true, false]);
	assert.equal(successCount, 0);
	assert.deepEqual(errors, [failure]);
});

test('reports a successful PDF download before clearing exporting state', async () => {
	const module = await import('../../src/lib/utils/staff-own-timetable-pdf.ts');
	const exportingStates = [];
	let successCount = 0;
	let errorCount = 0;

	await module.runStaffOwnTimetablePdfDownload(
		{ pages: [], fileName: 'ตารางสอน ครูสายใจ' },
		{
			generatePdf: async () => {},
			setExporting: (value) => exportingStates.push(value),
			onSuccess: () => {
				successCount += 1;
			},
			onError: () => {
				errorCount += 1;
			}
		}
	);

	assert.deepEqual(exportingStates, [true, false]);
	assert.equal(successCount, 1);
	assert.equal(errorCount, 0);
});
