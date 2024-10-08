use cpal::traits::DeviceTrait;
use cpal::SupportedStreamConfig;
use hound::WavWriter;
use std::fs::File;
use std::io::BufWriter;

/// Writes the input audio data to the WAV file, converting it to i16 format.
pub fn write_input_data<T>(
  input: &[T],
  writer: &mut WavWriter<BufWriter<File>>,
) where
  T: cpal::Sample,
{
  for &sample in input.iter() {
      let sample_i16 = sample.to_i16();
      writer.write_sample(sample_i16).unwrap_or_else(|e| {
          eprintln!("Failed to write sample: {}", e);
      });
  }
}

/// Helper function to find a supported audio configuration.
pub fn find_supported_config(device: &cpal::Device) -> Option<SupportedStreamConfig> {
  device.supported_input_configs().ok()?.find_map(|config_range| {
      let min_rate = config_range.min_sample_rate().0;
      let max_rate = config_range.max_sample_rate().0;

      [/*48000, 32000, */16000, 8000]
          .iter()
          .find(|&&rate| rate >= min_rate && rate <= max_rate)
          .map(|&rate| config_range.with_sample_rate(cpal::SampleRate(rate)))
  })
}