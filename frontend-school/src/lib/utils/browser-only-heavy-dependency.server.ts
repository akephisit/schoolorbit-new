function browserOnlyDependency(): never {
	throw new Error('ฟังก์ชันส่งออกและแปลงไฟล์ใช้งานได้เฉพาะในเบราว์เซอร์');
}

class BrowserOnlyWorkbook {
	constructor() {
		browserOnlyDependency();
	}
}

export const utils = {
	aoa_to_sheet: browserOnlyDependency,
	book_append_sheet: browserOnlyDependency,
	book_new: browserOnlyDependency,
	json_to_sheet: browserOnlyDependency,
	sheet_to_json: browserOnlyDependency
};

export const read = browserOnlyDependency;
export const writeFile = browserOnlyDependency;

export default Object.assign(browserOnlyDependency, {
	Workbook: BrowserOnlyWorkbook
});
