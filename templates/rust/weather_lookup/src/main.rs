use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize)]
struct WeatherInput {
    city: String,
}

#[derive(Serialize)]
struct WeatherOutput {
    city: String,
    temperature: f64,
    condition: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <json_input>", args[0]);
        std::process::exit(1);
    }

    let input: WeatherInput = serde_json::from_str(&args[1])?;
    
    // Mock implementation
    let output = WeatherOutput {
        city: input.city,
        temperature: 22.5,
        condition: "Sunny".to_string(),
    };

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
