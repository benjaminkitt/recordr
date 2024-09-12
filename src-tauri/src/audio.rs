// src-tauri/src/audio.rs

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::WavWriter;
use std::sync::{Arc, Mutex};

// Thread-local storage for recording state
thread_local! {
    static RECORDING: std::cell::RefCell<Option<(Stream, Arc<Mutex<WavWriter<std::io::BufWriter<std::fs::File>>>>)>> = std::cell::RefCell::new(None);
}

#[tauri::command]
pub fn start_recording(filename: String) -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();
        if recording.is_some() {
            return Err("Recording is already in progress".into());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;

        let spec = hound::WavSpec {
            channels: config.channels(),
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = hound::WavWriter::create(filename, spec).map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(writer));

        let writer_clone = Arc::clone(&writer);

        let err_fn = move |err| {
            eprintln!("An error occurred on stream: {}", err);
        };

        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data::<f32>(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data::<i16>(data, &mut *writer);
                },
                err_fn,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    let mut writer = writer_clone.lock().unwrap();
                    write_input_data::<u16>(data, &mut *writer);
                },
                err_fn,
            ),
            _ => return Err("Unsupported sample format".into()),
        }
        .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        *recording = Some((stream, writer));

        Ok("Recording started".into())
    })
}

#[tauri::command]
pub fn stop_recording() -> Result<String, String> {
    RECORDING.with(|recording| {
        let mut recording = recording.borrow_mut();
        if let Some((stream, _writer)) = recording.take() {
            drop(stream); // Stops the stream
            // _writer is dropped here, finalizing the WAV file
            Ok("Recording stopped".into())
        } else {
            Err("No recording in progress".into())
        }
    })
}

fn write_input_data<T>(
    input: &[T],
    writer: &mut hound::WavWriter<std::io::BufWriter<std::fs::File>>,
) where
    T: cpal::Sample,
{
    for &sample in input.iter() {
        let sample_i16 = sample.to_i16();
        writer.write_sample(sample_i16).unwrap();
    }
}
