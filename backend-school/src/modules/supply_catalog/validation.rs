use school_errors::AppError;
use unicode_normalization::UnicodeNormalization;

pub fn normalize(value: &str) -> String { value.nfkc().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase() }
pub fn text(value: &str, max: usize, required: bool) -> Result<String, AppError> {
 let value = value.trim();
 if (required && value.is_empty()) || value.chars().count() > max || value.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err(AppError::BadRequest("ข้อความว่างหรือยาวเกินกำหนด".into())); }
 Ok(value.to_owned())
}
pub fn parse_price(value: &str) -> Result<i64, AppError> {
 let parts: Vec<_> = value.split('.').collect();
 if parts.len()>2 || parts[0].is_empty() || parts[0].len()>10 || !parts[0].bytes().all(|b| b.is_ascii_digit()) { return Err(price_error()); }
 let fraction = parts.get(1).copied().unwrap_or("");
 if fraction.len()>2 || (parts.len()==2 && fraction.is_empty()) || !fraction.bytes().all(|b| b.is_ascii_digit()) { return Err(price_error()); }
 let whole=parts[0].parse::<i64>().map_err(|_|price_error())?;
 let cents=match fraction.len() { 0=>0,1=>fraction.parse::<i64>().map_err(|_|price_error())?*10,_=>fraction.parse::<i64>().map_err(|_|price_error())? };
 let result=whole.checked_mul(100).and_then(|v|v.checked_add(cents)).ok_or_else(price_error)?;
 if result>999999999999 { return Err(price_error()); } Ok(result)
}
fn price_error()->AppError { AppError::BadRequest("ราคาต้องเป็นจำนวนเงินไม่ติดลบและมีทศนิยมไม่เกินสองตำแหน่ง".into()) }
pub fn format_price(value:i64)->String { format!("{}.{:02}",value/100,value%100) }
pub fn paging(page:Option<i64>,size:Option<i64>)->Result<(i64,i64,i64),AppError> {
 let page=page.unwrap_or(1); let size=size.unwrap_or(24);
 if !(1..=100000).contains(&page)||!(1..=100).contains(&size) { return Err(AppError::BadRequest("หน้าหรือจำนวนรายการไม่ถูกต้อง".into())); } Ok((page,size,(page-1)*size))
}
pub fn revision(actual:i64,expected:i64)->Result<(),AppError> { if expected>0&&actual==expected {Ok(())}else{Err(AppError::Conflict("ข้อมูลเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into()))} }
pub fn db(error:sqlx::Error)->AppError { if let sqlx::Error::Database(ref e)=error { if e.is_unique_violation() { return AppError::Conflict("มีชื่อวัสดุและหน่วยนับนี้แล้ว หรือไฟล์ถูกใช้งานแล้ว".into()); } if e.is_foreign_key_violation() { return AppError::BadRequest("ไม่พบหมวดหมู่หรือวัสดุที่เลือก".into()); } } AppError::DbError(error) }
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn money_is_exact_and_bounded(){ for(v,n)in[("0",0),("1.2",120),("12.34",1234),("9999999999.99",999999999999)]{assert_eq!(parse_price(v).unwrap(),n);assert_eq!(parse_price(&format_price(n)).unwrap(),n);}for v in["-1","NaN","1e3","1.234","1.","10000000000.00"," 1",".2"]{assert!(parse_price(v).is_err(),"{v}");} }
 #[test] fn names_normalize_exact_duplicates(){assert_eq!(normalize("  PAPER \t A4 "),normalize("paper a4"));assert_eq!(normalize("Ａ４"),"a4");}
 #[test] fn revisions_and_paging_refuse_stale_or_unbounded(){assert!(revision(2,1).is_err());assert!(revision(1,0).is_err());assert!(paging(Some(0),None).is_err());assert!(paging(None,Some(101)).is_err());}
}
