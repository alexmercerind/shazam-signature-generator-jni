use chfft::RFft1D;
use std::collections::HashMap;

use crate::fingerprinting::hanning::HANNING_WINDOW_2048_MULTIPLIERS;
use crate::fingerprinting::signature_format::{DecodedSignature, FrequencyBand, FrequencyPeak};

pub struct SignatureGenerator {
    ring_buffer_of_samples: Vec<i16>,
    ring_buffer_of_samples_index: usize,

    reordered_ring_buffer_of_samples: Vec<f32>,

    fft_outputs: Vec<[f32; 1025]>,
    fft_outputs_index: usize,

    fft_object: RFft1D<f32>,

    spread_fft_outputs: Vec<[f32; 1025]>,
    spread_fft_outputs_index: usize,

    num_spread_ffts_done: u32,

    signature: DecodedSignature,
}

impl SignatureGenerator {
    pub fn make_signature_from_buffer(s16_mono_16khz_buffer: &[i16]) -> DecodedSignature {
        let mut this = SignatureGenerator {
            ring_buffer_of_samples: vec![0; 2048],
            ring_buffer_of_samples_index: 0,

            reordered_ring_buffer_of_samples: vec![0.0; 2048],

            fft_outputs: vec![[0.0; 1025]; 256],
            fft_outputs_index: 0,

            fft_object: RFft1D::<f32>::new(2048),

            spread_fft_outputs: vec![[0.0; 1025]; 256],
            spread_fft_outputs_index: 0,

            num_spread_ffts_done: 0,

            signature: DecodedSignature {
                sample_rate_hz: 16000,
                number_samples: s16_mono_16khz_buffer.len() as u32,
                frequency_band_to_sound_peaks: HashMap::new(),
            },
        };

        for chunk in s16_mono_16khz_buffer.chunks_exact(128) {
            this.do_fft(chunk);

            this.do_peak_spreading();

            this.num_spread_ffts_done += 1;

            if this.num_spread_ffts_done >= 46 {
                this.do_peak_recognition();
            }
        }

        this.signature
    }

    fn do_fft(self: &mut Self, s16_mono_16khz_buffer: &[i16]) {
        self.ring_buffer_of_samples
            [self.ring_buffer_of_samples_index..self.ring_buffer_of_samples_index + 128]
            .copy_from_slice(s16_mono_16khz_buffer);

        self.ring_buffer_of_samples_index += 128;
        self.ring_buffer_of_samples_index &= 2047;

        for index in 0..=2047 {
            self.reordered_ring_buffer_of_samples[index] = self.ring_buffer_of_samples
                [(index + self.ring_buffer_of_samples_index) & 2047]
                as f32
                * HANNING_WINDOW_2048_MULTIPLIERS[index];
        }

        let complex_fft_results = self
            .fft_object
            .forward(&self.reordered_ring_buffer_of_samples);

        assert_eq!(complex_fft_results.len(), 1025);

        let real_fft_results = &mut self.fft_outputs[self.fft_outputs_index];

        for index in 0..=1024 {
            real_fft_results[index] = ((complex_fft_results[index].re.powi(2)
                + complex_fft_results[index].im.powi(2))
                / ((1 << 17) as f32))
                .max(0.0000000001);
        }

        self.fft_outputs_index += 1;
        self.fft_outputs_index &= 255;
    }

    fn do_peak_spreading(self: &mut Self) {
        let real_fft_results =
            &self.fft_outputs[((self.fft_outputs_index as i32 - 1) & 255) as usize];

        let spread_fft_results = &mut self.spread_fft_outputs[self.spread_fft_outputs_index];

        spread_fft_results.copy_from_slice(real_fft_results);

        for position in 0..=1022 {
            spread_fft_results[position] = spread_fft_results[position]
                .max(spread_fft_results[position + 1])
                .max(spread_fft_results[position + 2]);
        }

        let spread_fft_results_copy = *spread_fft_results;

        for position in 0..=1024 {
            for former_fft_number in &[1, 3, 6] {
                let former_fft_output = &mut self.spread_fft_outputs
                    [((self.spread_fft_outputs_index as i32 - *former_fft_number) & 255) as usize];

                former_fft_output[position] =
                    former_fft_output[position].max(spread_fft_results_copy[position]);
            }
        }

        self.spread_fft_outputs_index += 1;
        self.spread_fft_outputs_index &= 255;
    }

    fn do_peak_recognition(self: &mut Self) {
        let fft_minus_46 = &self.fft_outputs[((self.fft_outputs_index as i32 - 46) & 255) as usize];
        let fft_minus_49 =
            &self.spread_fft_outputs[((self.spread_fft_outputs_index as i32 - 49) & 255) as usize];

        for bin_position in 10..=1014 {
            if fft_minus_46[bin_position] >= 1.0 / 64.0
                && fft_minus_46[bin_position] >= fft_minus_49[bin_position - 1]
            {
                let mut max_neighbor_in_fft_minus_49: f32 = 0.0;

                for neighbor_offset in &[-10, -7, -4, -3, 1, 2, 5, 8] {
                    max_neighbor_in_fft_minus_49 = max_neighbor_in_fft_minus_49
                        .max(fft_minus_49[(bin_position as i32 + *neighbor_offset) as usize]);
                }

                if fft_minus_46[bin_position] > max_neighbor_in_fft_minus_49 {
                    let mut max_neighbor_in_other_adjacent_ffts = max_neighbor_in_fft_minus_49;

                    for other_offset in &[
                        -53, -45, 165, 172, 179, 186, 193, 200, 214, 221, 228, 235, 242, 249,
                    ] {
                        let other_fft = &self.spread_fft_outputs[((self.spread_fft_outputs_index
                            as i32
                            + other_offset)
                            & 255)
                            as usize];

                        max_neighbor_in_other_adjacent_ffts =
                            max_neighbor_in_other_adjacent_ffts.max(other_fft[bin_position - 1]);
                    }

                    if fft_minus_46[bin_position] > max_neighbor_in_other_adjacent_ffts {
                        let fft_pass_number = self.num_spread_ffts_done - 46;

                        let peak_magnitude: f32 =
                            fft_minus_46[bin_position].ln().max(1.0 / 64.0) * 1477.3 + 6144.0;
                        let peak_magnitude_before: f32 =
                            fft_minus_46[bin_position - 1].ln().max(1.0 / 64.0) * 1477.3 + 6144.0;
                        let peak_magnitude_after: f32 =
                            fft_minus_46[bin_position + 1].ln().max(1.0 / 64.0) * 1477.3 + 6144.0;

                        let peak_variation_1: f32 =
                            peak_magnitude * 2.0 - peak_magnitude_before - peak_magnitude_after;
                        let peak_variation_2: f32 = (peak_magnitude_after - peak_magnitude_before)
                            * 32.0
                            / peak_variation_1;

                        let corrected_peak_frequency_bin: u16 =
                            bin_position as u16 * 64 + peak_variation_2 as u16;

                        assert!(peak_variation_1 >= 0.0);

                        let frequency_hz: f32 =
                            corrected_peak_frequency_bin as f32 * (16000.0 / 2.0 / 1024.0 / 64.0);

                        let frequency_band = match frequency_hz as i32 {
                            250..=519 => FrequencyBand::_250_520,
                            520..=1449 => FrequencyBand::_520_1450,
                            1450..=3499 => FrequencyBand::_1450_3500,
                            3500..=5500 => FrequencyBand::_3500_5500,
                            _ => {
                                continue;
                            }
                        };

                        if !self
                            .signature
                            .frequency_band_to_sound_peaks
                            .contains_key(&frequency_band)
                        {
                            self.signature
                                .frequency_band_to_sound_peaks
                                .insert(frequency_band, vec![]);
                        }

                        self.signature
                            .frequency_band_to_sound_peaks
                            .get_mut(&frequency_band)
                            .unwrap()
                            .push(FrequencyPeak {
                                fft_pass_number: fft_pass_number,
                                peak_magnitude: peak_magnitude as u16,
                                corrected_peak_frequency_bin: corrected_peak_frequency_bin,
                                sample_rate_hz: 16000,
                            });
                    }
                }
            }
        }
    }
}
