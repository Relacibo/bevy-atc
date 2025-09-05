use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use regex::Regex;
use serde_json;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

mod airlines;
mod aviation_command;
mod errors;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

const AIRLINES_JSON_PATH: &str = "resources/known-strings/airlines.json";
const ALPHABET_JSON_PATH: &str = "resources/known-strings/alphabet.json";
const MODEL_PATH: &str = "resources/models/ggml-medium.bin";

const SAMPLE_RATE_HZ: u32 = 16000;

const WINDOW_LEN_SECONDS: u32 = 20;
const CHECK_INTERVAL: u64 = 3000;

const PICKY: f32 = 0.95;
// const DESPERATE: f32 = 0.3; // Untergrenze für die Sicherheit

const WHISPER_NUM_THREADS: i32 = 2;
const WHISPER_MAX_SNIPPET_LEN_SECONDS: f32 = 17.0;

pub fn create_resampler(sample_rate_in: u32) -> SincFixedIn<f32> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.99,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let resample_ratio = SAMPLE_RATE_HZ as f64 / sample_rate_in as f64;
    SincFixedIn::<f32>::new(resample_ratio, 2.0, params, 1024, 1).unwrap()
}

fn main() -> Result<(), crate::errors::Error> {
    let cpal_host = cpal::default_host();
    let input_device = cpal_host
        .default_input_device()
        .ok_or(crate::errors::Error::FailedToFindDefaultInputDevice)?;

    #[cfg(debug_assertions)]
    if let Ok(name) = input_device.name() {
        println!("Using input device: {}", name);
    }

    // We'll try and use the same configuration between streams to keep it simple.
    let config: cpal::StreamConfig = input_device.default_input_config()?.into();

    let sample_rate_in = config.sample_rate.0;
    let channel_count_in = config.channels;

    // WINDOW_SIZE_SECONDS seconds
    let latency_samples = SAMPLE_RATE_HZ * WINDOW_LEN_SECONDS;
    // The buffer to share samples. We can buffer 16 seconds maximum.
    let ring = HeapRb::<f32>::new(latency_samples as usize * 8);
    let (mut producer, consumer) = ring.split();

    let resample_buffer: Arc<Mutex<[Vec<f32>; 1]>> = Arc::new(Mutex::new([vec![]]));

    // Fill the samples with 0.0 equal to the length of the delay.
    // for _ in 0..latency_samples {
    //     // The ring buffer has twice as much space as necessary to add latency here,
    //     // so this should never fail
    //     producer.try_push(0.0).unwrap();
    // }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let Ok(mut rb) = resample_buffer.lock() else {
            eprintln!("Could not lock mutex");
            return;
        };
        rb[0].clear();

        let data2 = if sample_rate_in != SAMPLE_RATE_HZ {
            let mut resampler = create_resampler(sample_rate_in);

            // Calculate the expected output length and resize the buffer
            let expected_output_len = ((data.len() as f64 / channel_count_in as f64)
                * (SAMPLE_RATE_HZ as f64 / sample_rate_in as f64))
                .ceil() as usize;
            rb[0].resize(expected_output_len, 0.0);

            if let Err(err) = resampler.process_into_buffer(
                &[data
                    .chunks(channel_count_in as usize)
                    .map(|frame| frame[0])
                    .collect::<Vec<_>>()],
                rb.as_mut_slice(),
                None,
            ) {
                eprintln!("Rubato resampling failed");
                dbg!(err);
                return;
            }
            &rb[0][..]
        } else {
            data
        };
        let len = data2.len();
        let pushed_count = producer.push_slice(data2);
        if len - pushed_count != 0 {
            eprintln!("Mic buffer overflow");
        }
    };
    let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
    let consumer_clone = Arc::new(Mutex::new(consumer));
    // Channel für erkannte Texte
    let (tx, rx) = mpsc::channel::<String>();
    let tx_thread = tx.clone();

    thread::spawn(move || {
        let ctx = WhisperContext::new_with_params(MODEL_PATH, WhisperContextParameters::default())
            .expect("failed to open model");
        let mut state = ctx.create_state().expect("failed to create key");
        let mut params = FullParams::new(SamplingStrategy::default());
        params.set_n_threads(WHISPER_NUM_THREADS);
        params.set_translate(false);
        params.set_language(Some("en"));

        let mut audio_buffer = vec![0.0f32; SAMPLE_RATE_HZ as usize * WINDOW_LEN_SECONDS as usize];

        loop {
            let read_samples_len = {
                let cons = consumer_clone.lock().unwrap();
                cons.peek_slice(&mut audio_buffer)
            };

            println!("Samples im Buffer: {}", read_samples_len);

            if read_samples_len < SAMPLE_RATE_HZ as usize {
                thread::sleep(std::time::Duration::from_millis(CHECK_INTERVAL));
                continue;
            }

            if state
                .full(params.clone(), &audio_buffer[..read_samples_len])
                .is_ok()
            {
                let text = state.full_get_segment_text(0).unwrap_or_default();
                let n_tokens = state.full_n_tokens(0).unwrap_or(0);
                let mut avg_prob = 0.0;
                if n_tokens > 0 {
                    let mut sum_prob = 0.0;
                    for i in 0..n_tokens {
                        if let Ok(prob) = state.full_get_token_prob(0, i) {
                            sum_prob += prob;
                        }
                    }
                    avg_prob = sum_prob / n_tokens as f32;
                }
                if !text.trim().is_empty() {
                    // Quadratically falling threshold
                    let x = read_samples_len as f32
                        / (SAMPLE_RATE_HZ as f32 * WHISPER_MAX_SNIPPET_LEN_SECONDS);
                    let threshold = PICKY - (PICKY * x);
                    if avg_prob > threshold {
                        let mut last_sentence_token: Option<i32> = None;
                        // Finde das letzte Satzende-Token (., ?, !, ...) im Text
                        for i in 0..n_tokens {
                            if let Ok(token) = state.full_get_token_text(0, i) {
                                if !token.starts_with("[_")
                                    && (token.ends_with('.')
                                        || token.ends_with('!')
                                        || token.ends_with('?'))
                                {
                                    last_sentence_token = Some(i);
                                    break;
                                }
                            }
                        }

                        let (text, len) = if let Some(i_token) = last_sentence_token {
                            let token_data = state.full_get_token_data(0, i_token).unwrap();
                            // t1 ist das Ende des Tokens in 10ms-Schritten relativ zum Segmentstart
                            let seconds = token_data.t1 as f32 * 0.01;
                            dbg!(token_data.t0);
                            dbg!(token_data.t1);
                            let used_samples = (seconds * SAMPLE_RATE_HZ as f32) as usize;

                            // Satztext bis zu diesem Token zusammensetzen
                            let mut sentence = String::new();
                            for i in 0..=i_token {
                                if let Ok(token) = state.full_get_token_text(0, i) {
                                    // Filter: Nur aufnehmen, wenn kein Whisper-Sondertoken
                                    if !token.starts_with("[_") && !token.ends_with("_]") {
                                        sentence.push_str(&token);
                                    }
                                }
                            }
                            let sentence = sentence.trim().to_string();
                            (sentence, used_samples)
                        } else {
                            // Kein Satzende gefunden, alles akzeptieren
                            (text, read_samples_len)
                        };
                        println!("Accepted (avg_p={:.2}): {}", avg_prob, text);
                        let mut cons = consumer_clone.lock().unwrap();
                        cons.skip(len);
                        drop(cons);

                        let _ = tx_thread.send(text);
                    } else {
                        println!(
                            "Uncertain recognition (avg_p={:.2}), waiting for more audio... (threshold={:.2})",
                            avg_prob, threshold
                        );
                    }
                }
            }
            thread::sleep(std::time::Duration::from_millis(CHECK_INTERVAL));
        }
    });

    input_stream.play()?;
    println!("Speak into the mic. Stop with Ctrl+C .");

    // Hier kannst du die erkannten Texte weiterverarbeiten:
    loop {
        if let Ok(recognized) = rx.recv() {
            println!("Received recognized text: {}", recognized);
            // Hier kannst du beliebige Weiterverarbeitung machen!
        }
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
