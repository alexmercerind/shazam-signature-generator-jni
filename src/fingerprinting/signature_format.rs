use byteorder::{LittleEndian, WriteBytesExt};
use crc32fast::Hasher;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Cursor, Seek, SeekFrom, Write};

const DATA_URI_PREFIX: &str = "data:audio/vnd.shazam.sig;base64,";

pub struct FrequencyPeak {
    pub fft_pass_number: u32,
    pub peak_magnitude: u16,
    pub corrected_peak_frequency_bin: u16,
    pub sample_rate_hz: u32,
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub enum FrequencyBand {
    _250_520 = 0,
    _520_1450 = 1,
    _1450_3500 = 2,
    _3500_5500 = 3,
}

impl Ord for FrequencyBand {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as i32).cmp(&(*other as i32))
    }
}

impl PartialOrd for FrequencyBand {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((*self as i32).cmp(&(*other as i32)))
    }
}

pub struct DecodedSignature {
    pub sample_rate_hz: u32,
    pub number_samples: u32,
    pub frequency_band_to_sound_peaks: HashMap<FrequencyBand, Vec<FrequencyPeak>>,
}

impl DecodedSignature {
    pub fn encode_to_binary(self: &Self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut cursor = Cursor::new(vec![]);

        cursor.write_u32::<LittleEndian>(0xcafe2580)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(0x94119c00)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(
            match self.sample_rate_hz {
                8000 => 1,
                11025 => 2,
                16000 => 3,
                32000 => 4,
                44100 => 5,
                48000 => 6,
                _ => {
                    panic!(
                        "{}",
                        "Invalid sample rate passed when encoding Shazam packet"
                    );
                }
            } << 27,
        )?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(0)?;
        cursor.write_u32::<LittleEndian>(
            self.number_samples + (self.sample_rate_hz as f32 * 0.24) as u32,
        )?;
        cursor.write_u32::<LittleEndian>((15 << 19) + 0x40000)?;

        cursor.write_u32::<LittleEndian>(0x40000000)?;
        cursor.write_u32::<LittleEndian>(0)?;

        let mut sorted_iterator: Vec<_> = self.frequency_band_to_sound_peaks.iter().collect();
        sorted_iterator.sort_by(|x, y| x.0.cmp(&y.0));

        for (frequency_band, frequency_peaks) in sorted_iterator {
            let mut peaks_cursor = Cursor::new(vec![]);

            let mut fft_pass_number = 0;

            for frequency_peak in frequency_peaks {
                assert!(frequency_peak.fft_pass_number >= fft_pass_number);

                if frequency_peak.fft_pass_number - fft_pass_number >= 255 {
                    peaks_cursor.write_u8(0xff)?;
                    peaks_cursor.write_u32::<LittleEndian>(frequency_peak.fft_pass_number)?;

                    fft_pass_number = frequency_peak.fft_pass_number;
                }

                peaks_cursor.write_u8((frequency_peak.fft_pass_number - fft_pass_number) as u8)?;

                peaks_cursor.write_u16::<LittleEndian>(frequency_peak.peak_magnitude)?;
                peaks_cursor
                    .write_u16::<LittleEndian>(frequency_peak.corrected_peak_frequency_bin)?;

                fft_pass_number = frequency_peak.fft_pass_number;
            }

            let peaks_buffer = peaks_cursor.into_inner();

            cursor.write_u32::<LittleEndian>(0x60030040 + *frequency_band as u32)?;
            cursor.write_u32::<LittleEndian>(peaks_buffer.len() as u32)?;
            cursor.write(&peaks_buffer)?;
            for _padding_index in 0..((4 - peaks_buffer.len() as u32 % 4) % 4) {
                cursor.write_u8(0)?;
            }
        }

        let buffer_size = cursor.position() as u32;

        cursor.seek(SeekFrom::Start(8))?;
        cursor.write_u32::<LittleEndian>(buffer_size - 48)?;

        cursor.seek(SeekFrom::Start(48 + 4))?;
        cursor.write_u32::<LittleEndian>(buffer_size - 48)?;

        cursor.seek(SeekFrom::Start(4))?;
        let mut hasher = Hasher::new();
        hasher.update(&cursor.get_ref()[8..]);
        cursor.write_u32::<LittleEndian>(hasher.finalize())?;

        Ok(cursor.into_inner())
    }

    pub fn encode_to_uri(self: &Self) -> Result<String, Box<dyn Error>> {
        Ok(format!(
            "{}{}",
            DATA_URI_PREFIX,
            base64::encode(self.encode_to_binary()?)
        ))
    }
}
