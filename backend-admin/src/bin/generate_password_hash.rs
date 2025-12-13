// Generate bcrypt password hash for migration
use bcrypt::{hash, DEFAULT_COST};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    let password = if args.len() > 1 {
        &args[1]
    } else {
        "test123"
    };

    let hash = hash(password, DEFAULT_COST)?;
    
    println!("Password: {}", password);
    println!("Bcrypt Hash:");
    println!("{}", hash);
    println!();
    println!("Use this in your migration file:");
    println!("'{}',", hash);

    Ok(())
}
