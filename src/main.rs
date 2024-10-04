extern crate ical;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::process::exit;

use log::*;

use clap::Parser;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

use serde::{Deserialize, Serialize};
use serde_json::Result;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Args {
    #[arg(short, long, help = "Path to the ics file")]
    path: Option<String>,
    #[arg(
        short,
        long,
        help = "Path to the output dir (Inkwatchy littlefs filesystem). With / at the end"
    )]
    output_dir: String,
    #[arg(
        short,
        long,
        help = "Limit how many days to actually show",
        default_value_t = 20
    )]
    limit_days: usize,
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "debug"),
    );
    debug!("Start");

    let args = Args::parse();

    if let Some(path) = args.path.clone() {
        let mut string = String::new();
        BufReader::new(File::open(path).unwrap())
            .read_to_string(&mut string)
            .unwrap();
        parse_ical(string.as_bytes(), &args.clone());
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    name: String,
    start_time: u64,
    end_time: u64,
    description: String,
    status: String,
}

pub fn parse_ical(buf: &[u8], args: &Args) {
    let mut reader = ical::IcalParser::new(buf);

    // Why can't i clone it, why does count consume it...
    if ical::IcalParser::new(buf).count() != 1 {
        error!("Ical count is not 1, something is weird");
    }

    let mut events: HashMap<String, Vec<Event>> = HashMap::new();
    let mut days: Vec<u64> = Vec::new();

    for i in reader.nth(0).unwrap().unwrap().events {
        debug!("I: {:#?}", i);
        let mut event = Event::default();
        let mut day = String::new();
        for p in i.properties {
            if p.name == "SUMMARY" {
                event.name = p.value.unwrap();
            } else if p.name == "DTSTART" {
                let time = NaiveDateTime::parse_from_str(&p.value.unwrap(), "%Y%m%dT%H%M%S")
                    .expect("Failed to parse date");

                day = time.format("%d.%m.%Y").to_string();
                let unix = Utc.from_utc_datetime(&time).timestamp() as u64;
                debug!("Unix time: {}", unix);
                event.start_time = unix;
            } else if p.name == "DTEND" {
                let time = NaiveDateTime::parse_from_str(&p.value.unwrap(), "%Y%m%dT%H%M%S")
                    .expect("Failed to parse date");

                let unix = Utc.from_utc_datetime(&time).timestamp() as u64;
                debug!("Unix time: {}", unix);
                event.end_time = unix;
            } else if p.name == "STATUS" {
                event.status = p.value.unwrap();
            } else if p.name == "DESCRIPTION" {
                event.description = p.value.unwrap();
            }
        }
        if let Some(map) = events.get_mut(&day) {
            map.push(event);
        } else {
            events.insert(day, vec![event.clone()]);
            days.push(event.start_time);
        }
    }

    debug!("Final hashmap: {:#?}", events);

    let json = serde_json::to_string_pretty(&events).unwrap();
    debug!("Json: \n{}", json);

    info!("There are {} days", events.len());
    for event in events {
        let path = format!("{}{}", args.output_dir, event.0);
        info!("Writing file: {}", path);
        let small_json = serde_json::to_string_pretty(&event).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(small_json.to_ascii_lowercase().as_bytes())
            .unwrap();
    }

    days.sort();
    debug!("Days: {:#?}", days);

    let mut index = String::new();

    let mut c = 0;
    for day in days {
        let time = DateTime::from_timestamp(day as i64, 0).unwrap();
        let time_str = &time.format("%d.%m.%Y").to_string();
        c = c + 1;
        if c > args.limit_days {
            let rm_path = format!("{}{}", args.output_dir, time_str);
            std::fs::remove_file(rm_path).unwrap();
            continue;
        }
        index.push_str(&day.to_string());
        index.push('\n');
    }

    let path = format!("{}{}", args.output_dir, "index.txt");
    info!("Writing file: {}", path);
    let mut file = File::create(path).unwrap();
    file.write_all(index.to_ascii_lowercase().as_bytes())
        .unwrap();

    info!("Done, bye!");
}
