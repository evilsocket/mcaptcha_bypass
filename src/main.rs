use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Instant;

use pow_sha256::ConfigBuilder;
use reqwest;
use serde::{Deserialize, Serialize};

static BANNER: &str = "
██ ███    ███      █████      ██████   ██████  ████████ 
██ ████  ████     ██   ██     ██   ██ ██    ██    ██    
██ ██ ████ ██     ███████     ██████  ██    ██    ██    
██ ██  ██  ██     ██   ██     ██   ██ ██    ██    ██    
██ ██      ██     ██   ██     ██████   ██████     ██";
static WEBSITE: &str = "http://localhost:7000";
static SITEKEY: &str = "9qO2b37Zy3A3oLp4VxwDyYizoRCO63Yp";
static THREADS: u32 = 50;

static TOT_SUCCESS: AtomicU32 = AtomicU32::new(0);
static TOT_ERRORS: AtomicU32 = AtomicU32::new(0);
static TOT_DIFFICULTY: AtomicU32 = AtomicU32::new(0);
static TOT_TIME_MS: AtomicU32 = AtomicU32::new(0);

#[derive(Deserialize, Debug)]
struct Config {
    difficulty_factor: u32,
    salt: String,
    string: String,
}

#[derive(Serialize)]
struct Verification {
    key: String,
    nonce: u64,
    result: String,
    string: String,
}

#[derive(Deserialize)]
struct Response {
    token: Option<String>,
    error: Option<String>,
}

fn main() {
    println!("{}\n", BANNER);

    let mut threads = Vec::new();

    println!("spawning {} threads ...", THREADS);

    let t_start = Instant::now();

    for _ in 0..THREADS {
        threads.push(thread::spawn(|| {
            let config_url = format!("{}/api/v1/pow/config", WEBSITE);
            let verify_url = format!("{}/api/v1/pow/verify", WEBSITE);

            // println!("fetching PoW configuration from {} ...", config_url);

            let mut map = HashMap::new();
            map.insert("key", SITEKEY);

            let client = reqwest::blocking::Client::new();

            let first_start = Instant::now();

            let config = client
                .post(config_url)
                .json(&map)
                .send()
                .unwrap()
                .json::<Config>()
                .unwrap();

            TOT_DIFFICULTY.fetch_add(config.difficulty_factor, Ordering::SeqCst);

            // let duration = first_start.elapsed();

            // println!("fetched in {:?}:\n\n{:#?}", duration, config);

            let pow_config = ConfigBuilder::default().salt(config.salt).build().unwrap();

            // let start = Instant::now();

            let work = pow_config
                .prove_work(&config.string, config.difficulty_factor)
                .unwrap();

            // let duration = start.elapsed();

            assert!(
                pow_config.calculate(&work, &config.string).unwrap()
                    >= config.difficulty_factor.into()
            );
            assert!(pow_config.is_valid_proof(&work, &config.string));
            assert!(pow_config.is_sufficient_difficulty(&work, config.difficulty_factor));

            /*
            println!(
                "\nsolved in {:?}:\n\n{:#?}\n\nverifying with {} ...",
                duration, work, verify_url
            );
            */

            let ver = Verification {
                key: SITEKEY.into(),
                nonce: work.nonce,
                result: work.result,
                string: config.string,
            };

            let resp = client
                .post(verify_url)
                .json(&ver)
                .send()
                .unwrap()
                .json::<Response>()
                .unwrap();

            let duration = first_start.elapsed();

            TOT_TIME_MS.fetch_add(duration.as_millis() as u32, Ordering::SeqCst);
            if resp.error.is_some() {
                // println!("verification error: {}", resp.error.unwrap());
                TOT_ERRORS.fetch_add(1, Ordering::SeqCst);
            } else {
                TOT_SUCCESS.fetch_add(1, Ordering::SeqCst);
                // println!("verified in {:?} token:'{}'", duration, resp.token.unwrap());
            }

            // println!("total time: {:?}", first_start.elapsed());
        }));
    }

    for thread in threads {
        if !thread.is_finished() {
            let res = thread.join();
            if res.is_err() {
                println!("JOIN ERROR: {:?}", res.err());
            }
        }
    }

    println!(
        "\n{} threads done in {:?}, verifications:{:?} errors:{:?} average_difficulty:{:?} average_verification_ms:{:?}",
        THREADS,
        t_start.elapsed(),
        TOT_SUCCESS,
        TOT_ERRORS,
        TOT_DIFFICULTY.load(Ordering::SeqCst) / THREADS,
        TOT_TIME_MS.load(Ordering::SeqCst) / THREADS
    );
}
