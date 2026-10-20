//! Integer arithmetic only; quantities are thousandths and rounding is half up per line.
use school_errors::AppError;
pub const MAX_MONEY: i64 = 999_999_999_999;
pub fn invalid(message: &str) -> AppError { AppError::ValidationError(message.into()) }
pub fn parse_money(value: &str) -> Result<i64, AppError> {
    if value.is_empty() || value.len() > 12 || !value.bytes().all(|b| b.is_ascii_digit()) { return Err(invalid("จำนวนเงินต้องเป็นสตางค์จำนวนเต็มไม่ติดลบ")); }
    let amount = value.parse::<i64>().map_err(|_|invalid("จำนวนเงินไม่ถูกต้อง"))?;
    if amount > MAX_MONEY { return Err(invalid("จำนวนเงินเกินขอบเขต")); }
    Ok(amount)
}
pub fn positive_money(value: &str) -> Result<i64, AppError> { let n=parse_money(value)?; if n==0 { return Err(invalid("จำนวนเงินต้องมากกว่าศูนย์")); } Ok(n) }
pub fn quantity_millis(value: &str) -> Result<i64,AppError> {
    let mut p=value.split('.'); let whole=p.next().unwrap_or(""); let fraction=p.next();
    if p.next().is_some() || whole.is_empty() || whole.len()>7 || !whole.bytes().all(|b|b.is_ascii_digit()) || fraction.is_some_and(|s|s.is_empty()||s.len()>3||!s.bytes().all(|b|b.is_ascii_digit())) { return Err(invalid("จำนวนต้องเป็นเลขบวกทศนิยมไม่เกินสามตำแหน่ง")); }
    let a=whole.parse::<i64>().map_err(|_|invalid("จำนวนไม่ถูกต้อง"))?;
    let b=if let Some(s)=fraction { s.parse::<i64>().map_err(|_|invalid("จำนวนไม่ถูกต้อง"))? * 10_i64.pow(3-s.len() as u32) } else {0};
    let n=a*1000+b;
    if !(1..=1_000_000_000).contains(&n) { return Err(invalid("จำนวนต้องมากกว่าศูนย์และไม่เกินหนึ่งล้าน")); } Ok(n)
}
pub fn line_total(quantity:&str,price:&str)->Result<i64,AppError> {
    let raw=i128::from(quantity_millis(quantity)?) * i128::from(parse_money(price)?);
    let result=(raw+500)/1000;
    if result>i128::from(MAX_MONEY) { return Err(invalid("ยอดรวมรายการเกินขอบเขต")); } Ok(result as i64)
}
pub fn sum(values:impl IntoIterator<Item=i64>)->Result<i64,AppError> {
    let n=values.into_iter().try_fold(0_i64,|a,b|a.checked_add(b).ok_or_else(||invalid("ยอดรวมเกินขอบเขต")))?;
    if n.unsigned_abs()>MAX_MONEY as u64 { return Err(invalid("ยอดรวมเกินขอบเขต")); } Ok(n)
}
pub fn text(value:&str,max:usize)->Result<String,AppError> { let s=value.trim(); if s.is_empty()||s.chars().count()>max||s.chars().any(|c|c.is_control()&&c!='\n'&&c!='\t') { return Err(invalid("ข้อความว่างหรือยาวเกินกำหนด")); } Ok(s.to_owned()) }
pub fn paging(query:&crate::models::BudgetListQuery)->Result<(i64,i64),AppError> { let l=query.limit.unwrap_or(30);let o=query.offset.unwrap_or(0);if !(1..=100).contains(&l)||!(0..=100000).contains(&o){return Err(invalid("หน้ารายการไม่ถูกต้อง"));}Ok((l,o)) }
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn exact_quantity_rounding_and_bounds(){ assert_eq!(line_total("1.005","100").unwrap(),101); assert_eq!(line_total("0.005","100").unwrap(),1); assert_eq!(line_total("0.004","100").unwrap(),0); assert_eq!(line_total("2","179400").unwrap(),358800); for q in ["0","-1","1.0001","1e2","1.",".1","1000000.001"]{assert!(quantity_millis(q).is_err(),"{q}");} for p in ["-1","1.00","1e2"," 1","1000000000000"]{assert!(parse_money(p).is_err(),"{p}");} assert!(line_total("1000000","999999999999").is_err()); }
 #[test] fn sum_refuses_overflow_and_request_paging_is_bounded(){assert!(sum([MAX_MONEY,1]).is_err());assert!(sum([i64::MAX,1]).is_err());assert!(paging(&crate::models::BudgetListQuery{limit:Some(101),..Default::default()}).is_err());}
}
