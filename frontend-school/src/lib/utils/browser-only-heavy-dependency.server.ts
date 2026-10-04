function browserOnlyDependency(): never {
	throw new Error('ฟังก์ชันส่งออกและแปลงไฟล์ใช้งานได้เฉพาะในเบราว์เซอร์');
}

class BrowserOnlyWorkbook {
	constructor() {
		browserOnlyDependency();
	}
}

export default Object.assign(browserOnlyDependency, {
	Workbook: BrowserOnlyWorkbook,
	format: browserOnlyDependency
});
