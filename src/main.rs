extern crate reqwest;
extern crate chrono;
extern crate dotenv;

use chrono::{NaiveDate, Days};
use chrono::prelude::*;
use dotenv::dotenv;
use serde_json::json;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // load env vars from .env (key and url)
    dotenv().ok();

    let date = get_date_arg()?;
    
    // get next date or handle last date edge case
    let next_date = if date == "December 31" { 
        String::from("STAYING STOIC") 
    } else { 
       increment_date(&date)
    };
    
    // get content url from env vars
    let url = env::var("daily_stoic_url")?;
    
    // fetch body from page and process it
    let body = fetch_page_body(&url)?;
    
    // get specific daily date text from body
    let date_text = get_date_text(&body, &date, &next_date)
        .ok_or("No match found")?;
    
    // format daily struct
    let mut daily: Daily = format_daily(&date_text);
    
    // fix quote
    daily.quote = fix_text_using_llm(&daily.quote)?;
    
    // fix explanation
    daily.explanation = fix_text_using_llm(&daily.explanation)?;
        
    println!("Date:\n{}\n", daily.date);
    println!("Title:\n{}\n", daily.title);
    println!("Quote:\n{}\n", daily.quote);
    println!("Quoter:\n{}\n", daily.quoter);
    println!("Explanation:\n{}", daily.explanation);

    Ok(())
}

fn get_date_arg() -> Result<String, String> {
    let args: Vec<String> = env::args().collect();
    
    // first arg is at args[2] 
    if args.len() < 3 {
        let today = Local::now()
            .date_naive()
            .with_year(2000)
            .unwrap(); // fixed to force leap year
        return Ok(today.format("%B %-d").to_string()); 
    } 
    
    let input = &args[2];
    let full_date = format!("{} 2000", input); // assume a leap year to get all possible days

    // verify valid date str
    let dt = NaiveDate::parse_from_str(&full_date, "%B %-d %Y")
        .map_err(|e| format!("Invalid date format for arg \"{}\" (must be %B %-d): {}", input, e))?;
    
    let parsed = dt.format("%B %-d").to_string();

    Ok(parsed)
}

fn fetch_page_body(url: &str) -> Result<String, String> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let body = response.text()
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    
    Ok(body)
}

fn get_date_text(text: &str, date: &str, next_date: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();

    // find the start
    let mut start = 0;
    for line in &lines {
        if line.starts_with(date) { break; }
        else { start += 1; }
    }
    
    // couldn't find date
    if start >= lines.len() { return None; }
    
    // find the end
    let mut end = start + 1;
    for line in &lines[end..] {
        if line.starts_with(&next_date) { break; } 
        else { end += 1; }
    }
    
    // couldn't find next date
    if end >= lines.len() { return None; } 
    
    let rejoined = lines[start..end].join("\n");
    Some(rejoined)
}

fn increment_date(date: &str) -> String {
    let full_date = format!("{} 2000", date); // assume a leap year to get all possible days
    let dt = NaiveDate::parse_from_str(&full_date, "%B %-d %Y").unwrap(); // date is already validated 
    let plus_one = dt + Days::new(1);
    plus_one.format("%B %-d").to_string()
}

struct Daily {
    date: String,
    title: String,
    quote: String,
    quoter: String,
    explanation: String 
}

fn format_daily(text: &str) -> Daily {
    let lines: Vec<&str> = text.lines().collect();

    let _date = lines[0].trim().to_string();
    let _title = lines[1].trim().to_string();
    
    let quote_start = 2;
    let mut quote_end = None;
    for (i, line) in lines[quote_start..].iter().enumerate() {
        if line.starts_with("—") {
            quote_end = Some(i + quote_start);
            break;
        }
    }

    let quote_end = quote_end.expect("Expected a line starting with — to end the quote");

    let _quote = lines[2..quote_end]
        .join(" ")
        .trim()
        .to_string();

    let _quoter = lines[quote_end]
        .trim()
        .to_string();

    let _explanation = lines[quote_end+1..]
        .join(" ")
        .trim()
        .to_string();

    Daily {
        date: _date,
        title: _title,
        quote: _quote,
        quoter: _quoter,
        explanation: _explanation
    }
}

fn fix_text_using_llm(text: &str) -> Result<String, String> {
    let endpoint = env::var("endpoint")
        .map_err(|e| format!("Failed to retrive endpoint from env vars: {}", e))?;

    let key = env::var("api_key")
        .map_err(|e| format!("Failed to retrive API key from env vars: {}", e))?;
    
    let max_tokens = 500;

    let client = reqwest::blocking::Client::new();

    let body = json!({
        "model": "openai/gpt-4o",
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Fix the text based on the following instructions:\n\
                    - Keep the quote as close to its original as possible.\n\
                    - Some words may be missing characters, combined together, or have a space in the middle of a word. Correct these.\n\
                    - Merge any line breaks that occur in the middle of a sentence.\n\
                    - Preserve paragraph breaks (indicated by empty lines or where appropriate).\n\
                    - Add an extra line break between paragraphs to improve readability.\n\
                    - Fix any missing characters or spacing issues in words.\n\
                    - Do not wrap the quote in quotation marks unless the text already has them.\n\
                    - If the line ends with a few lines with all caps that seem out of context, remove them.
                    - Do not add any commentary or explanation—just output the corrected quote.\n\
                    Text:\n{}",
                    text
                )
            }
        ],
        "max_tokens": max_tokens
    });

    let response = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", key))
        .json(&body)
        .send()
        .map_err(|e| format!("LLM request failed: {}", e))?;
   
    let response_json: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse LLM response JSON: {}", e))?;

    if let Some(error) = response_json.get("error") {
        if let Some(message) = error.get("message") { 
            return Err(format!("Request to format text with LLM resulted in an error: {}", message));
        } else { 
            return Err(format!("Request to format text with LLM resulted in an error and no message was found."));
        }
    } 
    
    let corrected_text = response_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Failed to extract content from LLM response")?
        .to_string();
    
    Ok(corrected_text)
}

