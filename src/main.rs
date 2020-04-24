use chrono::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let fname = args.next();
    let pattern = args.next();
    if pattern.is_none() {
        eprintln!("parameters: <log filename> <pattern>");
        std::process::exit(1);
    }
    let fname = fname.unwrap();
    let pattern = pattern.unwrap();

    let datetime_regex = Regex::new(r"^\d\d-\w{3}-\d{4} \d\d:\d\d:\d\d\.\d{3}").unwrap();
    let date_fmt = "%d-%b-%Y %T.%f %z";

    let file = File::open(fname)?;
    let reader = BufReader::new(file);

    let pattern_regex = Regex::new(&pattern).expect("Invalid pattern regex");

    let mut cur_range_start = None::<DateTime<Utc>>;
    let mut cur_range_end = None::<DateTime<Utc>>;
    let mut cur_timestamp = None::<DateTime<Utc>>;
    let mut match_count = 0;
    for line in reader.lines() {
        let line = line?;
        if datetime_regex.is_match(&line) {
            let timestamp_str = datetime_regex.find(&line).unwrap().as_str();
            cur_timestamp = Some(
                DateTime::parse_from_str(&(timestamp_str.to_string() + " +0000"), date_fmt)
                    .ok()
                    .unwrap()
                    .into(),
            );
            if cur_range_start.is_some()
                && cur_timestamp.unwrap() - cur_range_end.unwrap() < chrono::Duration::minutes(5)
            {
                // stays in the current range
            } else {
                if match_count > 0 {
                    println!(
                        "{} -> {}: {} matches",
                        cur_range_start.unwrap().format("%Y-%m-%d %T"),
                        cur_range_end.unwrap().format("%Y-%m-%d %T"),
                        match_count
                    );
                }
                cur_range_start = None;
                cur_range_end = None;
                match_count = 0;
            }
        }
        if pattern_regex.is_match(&line) {
            if cur_range_start.is_none() {
                cur_range_start = cur_timestamp.clone();
            }
            cur_range_end = cur_timestamp.clone();
            match_count += 1;
        }
    }
    Ok(())
}
