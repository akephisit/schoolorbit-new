import assert from 'node:assert/strict';
import test from 'node:test';

test('recognizes rotated A4 and preserves custom dimensions', async () => {
	const { describePaper } = await import('../../src/lib/certificates/paper.ts');

	assert.equal(
		describePaper({ widthPoints: 841.89, heightPoints: 595.28, rotation: 0 }),
		'A4 แนวนอน'
	);
	assert.equal(
		describePaper({ widthPoints: 595.28, heightPoints: 841.89, rotation: 90 }),
		'A4 แนวนอน'
	);
	assert.match(describePaper({ widthPoints: 720, heightPoints: 360, rotation: 0 }), /ขนาดกำหนดเอง/);
});

test('recognizes A5 and Letter within one millimetre', async () => {
	const { describePaper } = await import('../../src/lib/certificates/paper.ts');

	assert.equal(
		describePaper({ widthPoints: 419.53, heightPoints: 595.28, rotation: 0 }),
		'A5 แนวตั้ง'
	);
	assert.equal(
		describePaper({ widthPoints: 612.5, heightPoints: 791.5, rotation: 0 }),
		'Letter แนวตั้ง'
	);
});
