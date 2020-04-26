use chrono::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
mod dateformat;

struct InRangeState {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    pattern_idx: usize,
    match_count: usize,
}

enum ParsingState {
    NotInRange,
    InRange(InRangeState),
}

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let fname = args.next();
    if fname.as_deref() == Some("--version") {
        println!("version {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(1);
    }
    let patterns: Vec<String> = args.collect();
    if patterns.is_empty() {
        eprintln!("parameters: <log filename> <pattern> [extra patterns]");
        std::process::exit(1);
    }
    let fname = fname.unwrap();

    let file = File::open(fname)?;
    let reader = BufReader::new(file);

    let pattern_regexes: Vec<_> = patterns
        .iter()
        .map(|p| Regex::new(&p).expect("Invalid pattern regex"))
        .collect();

    let mut lines = reader.lines();
    let mut cur_timestamp = None::<DateTime<Utc>>;
    let mut state = ParsingState::NotInRange;
    let datefmt = match guess_dateformat(&mut lines)? {
        Some(f) => f,
        None => {
            eprintln!("Gave up guessing the date format");
            std::process::exit(1);
        }
    };

    for line in lines {
        let line = line?;
        // extract the timestamp from the line if present
        if let Some(timestamp_str) = datefmt.regex.find(&line).map(|m| m.as_str()) {
            let ts = (datefmt.parser)(timestamp_str);
            cur_timestamp = Some(ts);
            match state {
                ParsingState::InRange(ref st) if ts - st.end > chrono::Duration::minutes(5) => {
                    // too long interval, close the current range
                    print_pattern(&st, &patterns);
                    state = ParsingState::NotInRange;
                }
                _ => {}
            }
        }

        // if we have a timestamp, search for one of the patterns in the line
        if let Some(cur_timestamp) = cur_timestamp {
            match (
                pattern_regexes.iter().position(|p| p.is_match(&line)),
                &mut state,
            ) {
                (Some(idx), ParsingState::InRange(ref mut st)) if idx == st.pattern_idx => {
                    // prolonging the current pattern
                    st.end = cur_timestamp;
                    st.match_count += 1;
                }
                (Some(idx), ParsingState::InRange(st)) => {
                    // hit a different pattern
                    print_pattern(&st, &patterns);
                    state = ParsingState::InRange(InRangeState {
                        start: cur_timestamp,
                        end: cur_timestamp,
                        pattern_idx: idx,
                        match_count: 1,
                    });
                }
                (Some(idx), ParsingState::NotInRange) => {
                    state = ParsingState::InRange(InRangeState {
                        start: cur_timestamp,
                        end: cur_timestamp,
                        pattern_idx: idx,
                        match_count: 1,
                    });
                }
                (None, _) => {}
            }
        }
    }
    Ok(())
}

fn guess_dateformat(
    lines: &mut std::io::Lines<BufReader<File>>,
) -> std::io::Result<Option<dateformat::DateFormat>> {
    let mut datefmt_attempts = 0;
    for line in lines {
        let line = line?;

        let datefmt = dateformat::guess_date_format(&line);
        if datefmt.is_none() {
            datefmt_attempts += 1;
            if datefmt_attempts >= 5 {
                break;
            }
        } else {
            return Ok(datefmt);
        }
    }
    Ok(None)
}

fn print_pattern(state: &InRangeState, patterns: &[String]) {
    println!(
        "{} -> {}: [{}] {} matches",
        state.start.format("%Y-%m-%d %T"),
        state.end.format("%Y-%m-%d %T"),
        patterns[state.pattern_idx],
        state.match_count
    );
}
