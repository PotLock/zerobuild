use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize)]
struct CalcInput {
    a: f64,
    b: f64,
    operation: String,
}

#[derive(Serialize)]
struct CalcOutput {
    result: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <json_input>", args[0]);
        std::process::exit(1);
    }

    let input: CalcInput = serde_json::from_str(&args[1])?;
    
    let result = match input.operation.as_str() {
        "add" => input.a + input.b,
        "subtract" => input.a - input.b,
        "multiply" => input.a * input.b,
        "divide" => input.a / input.b,
        _ => 0.0,
    };

    let output = CalcOutput { result };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
