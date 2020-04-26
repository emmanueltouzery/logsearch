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
    // https://stackoverflow.com/a/49964042/516188
    let is_piped_input = !atty::is(atty::Stream::Stdin);
    let reader: Box<dyn BufRead> = if is_piped_input {
        // stdin is not a tty, reading from it
        Box::new(BufReader::new(std::io::stdin()))
    } else {
        // stdin is a tty, the first param must be a filename
        match args.next() {
            None => display_help_and_exit(),
            Some(fname) => {
                let file = File::open(fname)?;
                Box::new(BufReader::new(file))
            }
        }
    };

    // if the input is piped (eg the data may be coming in realtime)
    // and the output is not (we are displaying to a terminal), we will
    // display realtime progress info to the user, that we'll replace
    // with final data when we get it.
    let is_piped_output = !atty::is(atty::Stream::Stdout);
    let is_display_preview = is_piped_input && !is_piped_output;

    let patterns: Vec<String> = args.collect();
    if patterns.contains(&"--version".to_string()) {
        println!("version {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(1);
    }
    if patterns.is_empty() {
        display_help_and_exit();
    }

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
                    print_pattern(is_display_preview, &st, &patterns);
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
                    print_pattern(is_display_preview, &st, &patterns);
                    state = start_range(is_display_preview, idx, cur_timestamp, &patterns)?;
                }
                (Some(idx), ParsingState::NotInRange) => {
                    state = start_range(is_display_preview, idx, cur_timestamp, &patterns)?;
                }
                (None, _) => {}
            }
        }
    }
    if let ParsingState::InRange(st) = state {
        // print the final contents of the pattern when the input finishes
        print_pattern(is_display_preview, &st, &patterns);
    }
    Ok(())
}

fn display_help_and_exit() -> ! {
    eprintln!("parameters: <log filename> <pattern> [extra patterns]\nif data is passed by the standard input (piped in) then no need to pass log filename.");
    std::process::exit(1);
}

fn start_range(
    is_display_preview: bool,
    idx: usize,
    cur_timestamp: DateTime<Utc>,
    patterns: &[String],
) -> std::io::Result<ParsingState> {
    if is_display_preview {
        // this info may be overwritten later if the input is piped
        // hence we don't send a \n
        print!(
            "{} -> ? [{}] -- ongoing",
            cur_timestamp.format("%Y-%m-%d %T"),
            patterns[idx]
        );
        std::io::stdout().flush()?;
    }
    Ok(ParsingState::InRange(InRangeState {
        start: cur_timestamp,
        end: cur_timestamp,
        pattern_idx: idx,
        match_count: 1,
    }))
}

fn guess_dateformat(
    lines: &mut std::io::Lines<Box<dyn BufRead>>,
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

fn print_pattern(is_display_preview: bool, state: &InRangeState, patterns: &[String]) {
    if is_display_preview {
        // \r to clear the stdout if we had a piped output & progress report
        print!("\r");
    }
    println!(
        "{} -> {}: [{}] {} matches",
        state.start.format("%Y-%m-%d %T"),
        state.end.format("%Y-%m-%d %T"),
        patterns[state.pattern_idx],
        state.match_count
    );
}
