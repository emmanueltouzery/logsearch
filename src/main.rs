use chrono::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

struct ParsingState {
    cur_range_start: Option<DateTime<Utc>>,
    cur_range_end: Option<DateTime<Utc>>,
    cur_timestamp: Option<DateTime<Utc>>,
    cur_pattern: Option<usize>,
    match_count: usize,
}

impl ParsingState {
    fn new() -> ParsingState {
        ParsingState {
            cur_range_start: None,
            cur_range_end: None,
            cur_timestamp: None,
            cur_pattern: None,
            match_count: 0,
        }
    }
}

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let fname = args.next();
    let patterns: Vec<String> = args.collect();
    if patterns.is_empty() {
        eprintln!("parameters: <log filename> <pattern> [extra patterns]");
        std::process::exit(1);
    }
    let fname = fname.unwrap();

    let datetime_regex = Regex::new(r"^\d\d-\w{3}-\d{4} \d\d:\d\d:\d\d\.\d{3}").unwrap();
    let date_fmt = "%d-%b-%Y %T.%f %z";

    let file = File::open(fname)?;
    let reader = BufReader::new(file);

    let pattern_regexes: Vec<_> = patterns
        .iter()
        .map(|p| Regex::new(&p).expect("Invalid pattern regex"))
        .collect();

    let mut state = ParsingState::new();
    for line in reader.lines() {
        let line = line?;
        if datetime_regex.is_match(&line) {
            let timestamp_str = datetime_regex.find(&line).unwrap().as_str();
            state.cur_timestamp = Some(
                DateTime::parse_from_str(&(timestamp_str.to_string() + " +0000"), date_fmt)
                    .ok()
                    .unwrap()
                    .into(),
            );
            if state.cur_range_start.is_some()
                && state.cur_timestamp.unwrap() - state.cur_range_end.unwrap()
                    < chrono::Duration::minutes(5)
            {
                // stays in the current range
            } else {
                finish_pattern(&mut state, &patterns);
            }
        }
        match pattern_regexes.iter().position(|p| p.is_match(&line)) {
            Some(idx) if Some(idx) == state.cur_pattern => {
                // prolonging the current pattern
                increase_pattern(&mut state);
            }
            Some(idx) => {
                // hit a different pattern
                finish_pattern(&mut state, &patterns);
                state.cur_pattern = Some(idx);
                increase_pattern(&mut state);
            }
            _ => {}
        }
    }
    Ok(())
}

fn increase_pattern(state: &mut ParsingState) {
    if state.cur_range_start.is_none() {
        state.cur_range_start = state.cur_timestamp;
    }
    state.cur_range_end = state.cur_timestamp;
    state.match_count += 1;
}

fn finish_pattern(state: &mut ParsingState, patterns: &[String]) {
    if state.match_count > 0 {
        println!(
            "{} -> {}: [{}] {} matches",
            state.cur_range_start.unwrap().format("%Y-%m-%d %T"),
            state.cur_range_end.unwrap().format("%Y-%m-%d %T"),
            patterns[state.cur_pattern.unwrap()],
            state.match_count
        );
    }
    state.cur_range_start = None;
    state.cur_range_end = None;
    state.match_count = 0;
}
