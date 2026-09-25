//! WAV → PSAU audio cooker.
//!
//! This crate is host-only. It turns licensed source WAVs into
//! deterministic mono PS1 SPU ADPCM blobs and writes enough provenance
//! to reproduce the cook later. The ADPCM itself comes from the SDK's shared
//! encoder (`psx-audio-cook`): band-limited resampling, pre-compensation for
//! the SPU's Gaussian interpolation and a trellis search against the exact
//! SPU decoder, the same cook every game uses. [`spu_playback_wav`] renders a
//! cooked sound the way the console plays it, for previews.

use psx_audio_cook::{adpcm::FLAG_LOOP_START, CookOptions, Looping};
use psxed_format::audio::CODEC_SPU_ADPCM;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

const SPU_SAMPLE_RATE_HZ: u32 = 44_100;
/// One-shot effects do not need CD-quality bandwidth, and their cooked ADPCM
/// bytes remain resident in main RAM after the identical copy reaches SPU
/// RAM. Preserve already-lower-rate sources and cap higher-rate WAVs at the
/// PS1-appropriate 22.05 kHz tier instead of blindly upsampling everything to
/// 44.1 kHz.
const SFX_SAMPLE_RATE_CAP_HZ: u32 = 22_050;
const CDDA_SAMPLE_RATE_HZ: u32 = 44_100;
const CDDA_CHANNELS: usize = 2;
const CDDA_SECTOR_BYTES: usize = 2352;
const ADPCM_BLOCK_BYTES: usize = 16;
const ADPCM_SAMPLES_PER_BLOCK: usize = 28;

/// Options for a manifest-based import.
#[derive(Clone, Debug)]
pub struct PackOptions {
    /// Write decoded preview WAVs next to the cooked PSAU files.
    pub write_preview_wav: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            write_preview_wav: true,
        }
    }
}

/// JSON report emitted by a manifest import.
#[derive(Clone, Debug, Serialize)]
pub struct PackReport {
    /// Source pack display name.
    pub source_name: String,
    /// Source pack URL used for provenance.
    pub source_url: String,
    /// Source pack license identifier.
    pub license: String,
    /// SHA-256 of the imported source archive.
    pub archive_sha256: String,
    /// Target sample rate used for every cooked sound.
    pub target_sample_rate_hz: u32,
    /// Peak normalization target in `0.0..=1.0`.
    pub normalize_peak: f32,
    /// Per-sound cook results.
    pub sounds: Vec<SoundReport>,
}

/// JSON report for one cooked sound.
#[derive(Clone, Debug, Serialize)]
pub struct SoundReport {
    /// Stable asset id from the manifest.
    pub id: String,
    /// Path of the WAV inside the source archive.
    pub source_path: String,
    /// Relative cooked PSAU output path.
    pub output_psau: String,
    /// Relative decoded-preview WAV output path.
    pub preview_wav: Option<String>,
    /// SHA-256 of the source WAV bytes.
    pub source_sha256: String,
    /// SHA-256 of the cooked PSAU bytes.
    pub cooked_sha256: String,
    /// Source WAV sample rate.
    pub original_sample_rate_hz: u32,
    /// Source WAV channel count.
    pub original_channels: u16,
    /// Source frames before downmix/resample.
    pub input_sample_frames: u32,
    /// Audible mono sample count after cooking.
    pub cooked_sample_count: u32,
    /// Number of SPU ADPCM blocks.
    pub adpcm_blocks: u32,
    /// Cooked PSAU byte count.
    pub psau_bytes: usize,
    /// Peak before normalization in `0.0..=1.0`.
    pub peak_before: f32,
    /// Peak after normalization in `0.0..=1.0`.
    pub peak_after: f32,
    /// RMS error between normalized PCM and decoded ADPCM.
    pub decoded_rms_error: f32,
}

/// Errors surfaced by the audio cooker.
#[derive(Debug)]
pub enum Error {
    /// Filesystem operation failed.
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Manifest JSON could not be parsed or written.
    Json(serde_json::Error),
    /// Zip archive container could not be read.
    Zip(zip::result::ZipError),
    /// Required zip entry was not present.
    ZipEntry {
        /// Entry path from the manifest.
        path: String,
        /// Zip lookup error.
        source: zip::result::ZipError,
    },
    /// Archive hash differs from the manifest.
    ArchiveHashMismatch {
        /// Manifest hash.
        expected: String,
        /// Local archive hash.
        actual: String,
    },
    /// Manifest content is structurally invalid.
    InvalidManifest(String),
    /// WAV data is malformed or unsupported.
    Wav(String),
    /// ADPCM byte stream is malformed.
    Adpcm(String),
    /// Source audio had no samples after decoding.
    EmptyAudio,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Json(e) => write!(f, "json: {e}"),
            Error::Zip(e) => write!(f, "zip: {e}"),
            Error::ZipEntry { path, source } => write!(f, "zip entry {path}: {source}"),
            Error::ArchiveHashMismatch { expected, actual } => {
                write!(
                    f,
                    "archive sha256 mismatch: expected {expected}, got {actual}"
                )
            }
            Error::InvalidManifest(msg) => write!(f, "manifest: {msg}"),
            Error::Wav(msg) => write!(f, "wav: {msg}"),
            Error::Adpcm(msg) => write!(f, "adpcm: {msg}"),
            Error::EmptyAudio => write!(f, "source audio is empty"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PackManifest {
    source: SourceManifest,
    target_sample_rate_hz: Option<u32>,
    normalize_peak: Option<f32>,
    sounds: Vec<SoundManifest>,
}

#[derive(Clone, Debug, Deserialize)]
struct SourceManifest {
    name: String,
    url: String,
    license: String,
    archive_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SoundManifest {
    id: String,
    path: String,
}

#[derive(Copy, Clone, Debug)]
struct CookConfig {
    sample_rate_hz: u32,
    normalize_peak: f32,
}

#[derive(Clone, Debug)]
struct CookedAudio {
    psau: Vec<u8>,
    report: CookReport,
}

#[derive(Clone, Debug)]
struct CookReport {
    original_sample_rate_hz: u32,
    original_channels: u16,
    input_sample_frames: u32,
    cooked_sample_count: u32,
    adpcm_blocks: u32,
    peak_before: f32,
    peak_after: f32,
    decoded_rms_error: f32,
}

#[derive(Clone, Debug)]
struct WavPcm {
    sample_rate_hz: u32,
    channels: u16,
    samples: Vec<i16>,
}

/// Raw SPU ADPCM cooked from a WAV for direct bank assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpuAdpcmCook {
    /// Source WAV sample rate.
    pub source_sample_rate_hz: u32,
    /// Source WAV channel count before downmixing.
    pub source_channels: u16,
    /// Source PCM frame count before downmixing or resampling.
    pub source_sample_count: u32,
    /// Output sample rate represented by the ADPCM stream.
    pub sample_rate_hz: u32,
    /// Audible mono PCM samples before final block padding.
    pub sample_count: u32,
    /// Resampled loop start, when the source declares one externally.
    pub loop_start: Option<u32>,
    /// Raw 16-byte SPU ADPCM blocks.
    pub adpcm: Vec<u8>,
}

/// Import a source zip using a checked-in JSON manifest.
pub fn import_pack(
    manifest_path: &Path,
    archive_path: &Path,
    out_dir: &Path,
    options: &PackOptions,
) -> Result<PackReport, Error> {
    let manifest_bytes = read_file(manifest_path)?;
    let archive_bytes = read_file(archive_path)?;
    let manifest: PackManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_manifest(&manifest)?;

    let archive_sha256 = sha256_hex(&archive_bytes);
    if manifest.source.archive_sha256.to_ascii_lowercase() != archive_sha256 {
        return Err(Error::ArchiveHashMismatch {
            expected: manifest.source.archive_sha256,
            actual: archive_sha256,
        });
    }

    let sample_rate_hz = manifest.target_sample_rate_hz.unwrap_or(SPU_SAMPLE_RATE_HZ);
    if sample_rate_hz == 0 {
        return Err(Error::InvalidManifest(
            "target_sample_rate_hz must be non-zero".to_string(),
        ));
    }
    let normalize_peak = manifest.normalize_peak.unwrap_or(0.9).clamp(0.0, 1.0);
    let cfg = CookConfig {
        sample_rate_hz,
        normalize_peak,
    };

    let psau_dir = out_dir.join("psau");
    let preview_dir = out_dir.join("preview");
    create_dir(&psau_dir)?;
    if options.write_preview_wav {
        create_dir(&preview_dir)?;
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(archive_bytes.as_slice()))?;
    let mut reports = Vec::with_capacity(manifest.sounds.len());

    for sound in &manifest.sounds {
        let id = checked_asset_id(&sound.id)?;
        let wav_bytes = read_zip_entry(&mut archive, &sound.path)?;
        let cooked = cook_wav(&wav_bytes, &cfg)?;

        let psau_rel = format!("psau/{id}.psau");
        let psau_path = out_dir.join(&psau_rel);
        write_file(&psau_path, &cooked.psau)?;

        let preview_rel = if options.write_preview_wav {
            let rel = format!("preview/{id}.wav");
            let wav = spu_playback_wav(&cooked.psau)?;
            write_file(&out_dir.join(&rel), &wav)?;
            Some(rel)
        } else {
            None
        };

        reports.push(SoundReport {
            id,
            source_path: sound.path.clone(),
            output_psau: psau_rel,
            preview_wav: preview_rel,
            source_sha256: sha256_hex(&wav_bytes),
            cooked_sha256: sha256_hex(&cooked.psau),
            original_sample_rate_hz: cooked.report.original_sample_rate_hz,
            original_channels: cooked.report.original_channels,
            input_sample_frames: cooked.report.input_sample_frames,
            cooked_sample_count: cooked.report.cooked_sample_count,
            adpcm_blocks: cooked.report.adpcm_blocks,
            psau_bytes: cooked.psau.len(),
            peak_before: cooked.report.peak_before,
            peak_after: cooked.report.peak_after,
            decoded_rms_error: cooked.report.decoded_rms_error,
        });
    }

    let report = PackReport {
        source_name: manifest.source.name,
        source_url: manifest.source.url,
        license: manifest.source.license,
        archive_sha256,
        target_sample_rate_hz: cfg.sample_rate_hz,
        normalize_peak: cfg.normalize_peak,
        sounds: reports,
    };
    let report_json = serde_json::to_vec_pretty(&report)?;
    write_file(&out_dir.join("report.json"), &report_json)?;
    Ok(report)
}

/// Convert a source WAV into a raw CD-DA track payload.
///
/// Output is 44.1 kHz stereo signed 16-bit little-endian PCM with no
/// container header, padded to a whole number of 2352-byte CD-DA sectors.
pub fn cook_cdda_track_from_wav(src: &[u8]) -> Result<Vec<u8>, Error> {
    cook_cdda_track_from_wav_at_speed(src, 4096)
}

/// Convert a source WAV into a raw CD-DA track payload, baking a Q12 playback
/// speed multiplier into the sample data (`4096` = 1.0x).
pub fn cook_cdda_track_from_wav_at_speed(
    src: &[u8],
    playback_speed_q12: u16,
) -> Result<Vec<u8>, Error> {
    let wav = parse_wav_pcm(src)?;
    let mut stereo = wav_to_stereo_interleaved(&wav);
    if stereo.is_empty() {
        return Err(Error::EmptyAudio);
    }
    stereo = resample_stereo_linear_at_speed(
        &stereo,
        wav.sample_rate_hz,
        CDDA_SAMPLE_RATE_HZ,
        playback_speed_q12.max(1),
    );

    let mut out = Vec::with_capacity(stereo.len() * 2);
    for sample in stereo {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    let pad = (CDDA_SECTOR_BYTES - (out.len() % CDDA_SECTOR_BYTES)) % CDDA_SECTOR_BYTES;
    out.resize(out.len() + pad, 0);
    Ok(out)
}

/// Convert a source WAV into a cooked `.psau` one-shot sample for SPU SFX.
///
/// The output is mono SPU ADPCM wrapped in the shared PSoXide audio asset
/// header. Source rates at or below 22.05 kHz are preserved; higher-rate
/// sources are downsampled to 22.05 kHz. Playback duration and pitch remain
/// exact because the selected rate is stored in the PSAU header. Peak
/// normalization remains 0.9.
pub fn cook_sfx_from_wav(src: &[u8]) -> Result<Vec<u8>, Error> {
    let source = read_mono(src)?;
    let cooked = cook_wav(
        src,
        &CookConfig {
            sample_rate_hz: source.rate.min(SFX_SAMPLE_RATE_CAP_HZ),
            normalize_peak: 0.9,
        },
    )?;
    Ok(cooked.psau)
}

/// Cook a WAV into raw mono SPU ADPCM at a caller-selected sample rate.
///
/// This is the shared low-level path for games that assemble their own sound
/// banks instead of PSAU assets. `source_loop_start` is measured in source WAV
/// frames; the loop is moved to the next ADPCM block boundary (the lead-in is
/// padded by under a block) and repeats whole blocks, so it is seamless. The
/// source level is kept (no normalisation).
pub fn cook_spu_adpcm_from_wav(
    src: &[u8],
    sample_rate_hz: u32,
    source_loop_start: Option<u32>,
) -> Result<SpuAdpcmCook, Error> {
    if sample_rate_hz == 0 {
        return Err(Error::InvalidManifest(
            "SPU ADPCM sample rate must be non-zero".to_string(),
        ));
    }
    let parsed = parse_wav_pcm(src)?;
    let mut wav = read_mono(src)?;
    // The caller's loop point decides looping, not the file's own metadata.
    wav.loop_start = source_loop_start
        .map(|s| s as usize)
        .filter(|&s| s < wav.samples.len());
    wav.loop_end = None;
    let cooked = psx_audio_cook::cook(
        &wav,
        &CookOptions {
            rate: sample_rate_hz,
            normalize_peak: None,
            looping: Looping::Source,
            compensate_gauss: true,
            encode: Default::default(),
        },
    );
    let mut adpcm = cooked.adpcm;
    // A one-shot also latches its first block as the repeat address, as the
    // runtimes that play these banks have always read it.
    if cooked.loop_block.is_none() {
        adpcm[1] |= FLAG_LOOP_START;
    }
    Ok(SpuAdpcmCook {
        source_sample_rate_hz: parsed.sample_rate_hz,
        source_channels: parsed.channels,
        source_sample_count: (parsed.samples.len() / usize::from(parsed.channels)) as u32,
        sample_rate_hz,
        sample_count: cooked.pcm.len() as u32,
        loop_start: cooked
            .loop_block
            .map(|b| (b * ADPCM_SAMPLES_PER_BLOCK) as u32),
        adpcm,
    })
}

fn validate_manifest(manifest: &PackManifest) -> Result<(), Error> {
    if manifest.sounds.is_empty() {
        return Err(Error::InvalidManifest(
            "sounds must not be empty".to_string(),
        ));
    }
    if manifest.source.archive_sha256.len() != 64 {
        return Err(Error::InvalidManifest(
            "source.archive_sha256 must be a 64-character hex string".to_string(),
        ));
    }
    Ok(())
}

fn cook_wav(src: &[u8], cfg: &CookConfig) -> Result<CookedAudio, Error> {
    let parsed = parse_wav_pcm(src)?;
    let input_sample_frames = parsed.samples.len() / parsed.channels as usize;
    let wav = read_mono(src)?;
    let cooked = psx_audio_cook::cook(
        &wav,
        &CookOptions {
            rate: cfg.sample_rate_hz,
            normalize_peak: Some(cfg.normalize_peak as f64),
            looping: Looping::None,
            compensate_gauss: true,
            encode: Default::default(),
        },
    );
    let peak_before = wav.samples.iter().fold(0.0f64, |m, v| m.max(v.abs())) / 32_768.0;
    let peak_after = peak_ratio(&cooked.pcm);
    let mut decoded_preview = cooked.decoded();
    decoded_preview.truncate(cooked.pcm.len());
    let psau = psx_audio_cook::psau(cfg.sample_rate_hz, cooked.pcm.len(), &cooked.adpcm);
    let decoded_rms_error = rms_error(&cooked.pcm, &decoded_preview);

    Ok(CookedAudio {
        psau,
        report: CookReport {
            original_sample_rate_hz: parsed.sample_rate_hz,
            original_channels: parsed.channels,
            input_sample_frames: input_sample_frames as u32,
            cooked_sample_count: cooked.pcm.len() as u32,
            adpcm_blocks: (cooked.adpcm.len() / ADPCM_BLOCK_BYTES) as u32,
            peak_before: (peak_before as f32).min(1.0),
            peak_after,
            decoded_rms_error,
        },
    })
}

/// A WAV as the shared encoder reads it: mono, at its own rate.
fn read_mono(src: &[u8]) -> Result<psx_audio_cook::Wav, Error> {
    let wav = psx_audio_cook::wav::read(src).map_err(|e| Error::Wav(e.to_string()))?;
    if wav.samples.is_empty() {
        return Err(Error::EmptyAudio);
    }
    Ok(wav)
}

/// Render a cooked `.psau` as the console plays it: the ADPCM decoded as the
/// SPU decodes it and played through a model of the voice's Gaussian
/// interpolation, at 44.1 kHz. Returns a 16-bit mono WAV, so an editor
/// preview is what the game will sound like rather than the source file.
pub fn spu_playback_wav(psau: &[u8]) -> Result<Vec<u8>, Error> {
    if psau.len() < 32 + ADPCM_BLOCK_BYTES || &psau[0..4] != b"PSAU" {
        return Err(Error::Adpcm("not a PSAU asset".to_string()));
    }
    if psau[12] != CODEC_SPU_ADPCM {
        return Err(Error::Adpcm("PSAU codec is not SPU ADPCM".to_string()));
    }
    let rate = read_u32(psau, 16)?;
    let sample_count = read_u32(psau, 20)? as usize;
    if rate == 0 {
        return Err(Error::Adpcm("PSAU sample rate is zero".to_string()));
    }
    let adpcm = &psau[32..];
    let adpcm = &adpcm[..adpcm.len() / ADPCM_BLOCK_BYTES * ADPCM_BLOCK_BYTES];
    let mut decoded = psx_audio_cook::adpcm::decode(adpcm);
    decoded.truncate(sample_count.max(1));
    let played = psx_audio_cook::spu_play::play(&decoded, rate);
    Ok(write_wav_mono_i16(SPU_SAMPLE_RATE_HZ, &played))
}

fn parse_wav_pcm(bytes: &[u8]) -> Result<WavPcm, Error> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(Error::Wav("expected RIFF/WAVE header".to_string()));
    }

    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = read_u32(bytes, pos + 4)? as usize;
        let start = pos + 8;
        let end = start
            .checked_add(len)
            .ok_or_else(|| Error::Wav("chunk length overflow".to_string()))?;
        if end > bytes.len() {
            return Err(Error::Wav("chunk extends past EOF".to_string()));
        }

        match id {
            b"fmt " => {
                if len < 16 {
                    return Err(Error::Wav("fmt chunk too short".to_string()));
                }
                let audio_format = read_u16(bytes, start)?;
                let channels = read_u16(bytes, start + 2)?;
                let sample_rate_hz = read_u32(bytes, start + 4)?;
                let bits_per_sample = read_u16(bytes, start + 14)?;
                fmt = Some((audio_format, channels, sample_rate_hz, bits_per_sample));
            }
            b"data" => {
                data = Some(&bytes[start..end]);
                // Audio decoding needs no chunks after the PCM payload. Some
                // legacy game WAVs have malformed editor metadata after data.
                break;
            }
            _ => {}
        }

        pos = end + (len & 1);
    }

    let (audio_format, channels, sample_rate_hz, bits_per_sample) =
        fmt.ok_or_else(|| Error::Wav("missing fmt chunk".to_string()))?;
    if audio_format != 1 {
        return Err(Error::Wav(format!(
            "unsupported WAV format {audio_format}; expected PCM"
        )));
    }
    if channels == 0 {
        return Err(Error::Wav("channel count is zero".to_string()));
    }
    if bits_per_sample != 8 && bits_per_sample != 16 {
        return Err(Error::Wav(format!(
            "unsupported bit depth {bits_per_sample}; expected 8 or 16"
        )));
    }
    let data = data.ok_or_else(|| Error::Wav("missing data chunk".to_string()))?;
    let bytes_per_sample = usize::from(bits_per_sample / 8);
    let frame_bytes = usize::from(channels)
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| Error::Wav("WAV frame size overflow".to_string()))?;
    if !data.len().is_multiple_of(frame_bytes) {
        return Err(Error::Wav(
            "data chunk does not contain whole PCM frames".to_string(),
        ));
    }

    let samples = if bits_per_sample == 8 {
        data.iter()
            .map(|&sample| (i16::from(sample) - 128) << 8)
            .collect()
    } else {
        data.chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    };

    Ok(WavPcm {
        sample_rate_hz,
        channels,
        samples,
    })
}

fn wav_to_stereo_interleaved(wav: &WavPcm) -> Vec<i16> {
    let channels = wav.channels as usize;
    wav.samples
        .chunks_exact(channels)
        .flat_map(|frame| {
            let left = frame[0];
            let right = if channels > 1 { frame[1] } else { left };
            [left, right]
        })
        .collect()
}

fn resample_stereo_linear_at_speed(
    samples: &[i16],
    source_rate: u32,
    target_rate: u32,
    playback_speed_q12: u16,
) -> Vec<i16> {
    let frames = samples.len() / CDDA_CHANNELS;
    let speed = playback_speed_q12.max(1) as u64;
    if (source_rate == target_rate && speed == 4096) || frames < 2 {
        return samples.to_vec();
    }
    let source_rate_scaled = source_rate as u64 * speed;
    let out_num = frames as u64 * target_rate as u64 * 4096;
    let out_frames = (out_num + source_rate_scaled / 2) / source_rate_scaled;
    let out_frames = out_frames.max(1) as usize;
    let mut out = Vec::with_capacity(out_frames * CDDA_CHANNELS);

    for i in 0..out_frames {
        let pos =
            (i as f64) * (source_rate as f64) * (speed as f64) / (target_rate as f64 * 4096.0);
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(frames - 1);
        let t = pos - lo as f64;
        for ch in 0..CDDA_CHANNELS {
            let a = samples[lo * CDDA_CHANNELS + ch] as f64;
            let b = samples[hi * CDDA_CHANNELS + ch] as f64;
            out.push(
                (a + (b - a) * t)
                    .round()
                    .clamp(i16::MIN as f64, i16::MAX as f64) as i16,
            );
        }
    }
    out
}

fn peak_ratio(samples: &[i16]) -> f32 {
    let peak = samples.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);
    (peak as f32 / 32768.0).min(1.0)
}

fn write_wav_mono_i16(sample_rate_hz: u32, samples: &[i16]) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_bytes as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate_hz.to_le_bytes());
    out.extend_from_slice(&(sample_rate_hz * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn rms_error(reference: &[i16], decoded: &[i16]) -> f32 {
    let n = reference.len().min(decoded.len());
    if n == 0 {
        return 0.0;
    }
    let sum = reference
        .iter()
        .zip(decoded.iter())
        .take(n)
        .map(|(&a, &b)| {
            let d = a as f64 - b as f64;
            d * d
        })
        .sum::<f64>();
    (sum / n as f64).sqrt() as f32
}

fn checked_asset_id(id: &str) -> Result<String, Error> {
    let id = id.trim();
    if id.is_empty() {
        return Err(Error::InvalidManifest("sound id is empty".to_string()));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::InvalidManifest(format!(
            "sound id {id:?} must use ASCII letters, digits, '_' or '-'"
        )));
    }
    Ok(id.to_string())
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, Error> {
    let mut file = archive.by_name(path).map_err(|source| Error::ZipEntry {
        path: path.to_string(),
        source,
    })?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes).map_err(|source| Error::Io {
        path: PathBuf::from(path),
        source,
    })?;
    Ok(bytes)
}

fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    std::fs::write(path, bytes).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_dir(path: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{b:02x}").expect("write to String");
    }
    out
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error::Wav("unexpected EOF".to_string()))?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let word = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::Wav("unexpected EOF".to_string()))?;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_wav_roundtrips_through_parser() {
        let src = [0i16, 1200, -1200, 3200];
        let wav = write_wav_mono_i16(22_050, &src);
        let parsed = parse_wav_pcm(&wav).unwrap();

        assert_eq!(parsed.sample_rate_hz, 22_050);
        assert_eq!(parsed.channels, 1);
        assert_eq!(parsed.samples, src);
    }

    #[test]
    fn unsigned_eight_bit_pcm_expands_to_signed_sixteen_bit() {
        let src = [0u8, 64, 128, 192, 255];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + src.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&11_025u32.to_le_bytes());
        wav.extend_from_slice(&11_025u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(src.len() as u32).to_le_bytes());
        wav.extend_from_slice(&src);

        let parsed = parse_wav_pcm(&wav).unwrap();
        assert_eq!(parsed.sample_rate_hz, 11_025);
        assert_eq!(parsed.channels, 1);
        assert_eq!(parsed.samples, [-32_768, -16_384, 0, 16_384, 32_512]);
    }

    #[test]
    fn silence_cooks_to_silent_adpcm() {
        let wav = write_wav_mono_i16(11_025, &[0i16; ADPCM_SAMPLES_PER_BLOCK * 2]);
        let cooked = cook_spu_adpcm_from_wav(&wav, 11_025, None).unwrap();
        let decoded = psx_audio_cook::adpcm::decode(&cooked.adpcm);

        assert_eq!(cooked.adpcm.len(), ADPCM_BLOCK_BYTES * 2);
        assert_eq!(cooked.adpcm[17], 0x01);
        assert!(decoded.iter().all(|&s| s == 0));
    }

    #[test]
    fn cook_wav_emits_psau_header_and_preview() {
        let samples: Vec<i16> = (0..128).map(|i| (((i % 32) - 16) * 512) as i16).collect();
        let wav = write_wav_mono_i16(44_100, &samples);
        let cooked = cook_wav(
            &wav,
            &CookConfig {
                sample_rate_hz: 44_100,
                normalize_peak: 0.9,
            },
        )
        .unwrap();

        assert_eq!(&cooked.psau[0..4], b"PSAU");
        assert_eq!(u16::from_le_bytes([cooked.psau[4], cooked.psau[5]]), 1);
        assert_eq!(cooked.psau[12], CODEC_SPU_ADPCM);
        assert_eq!(cooked.psau[13], 1);
        assert_eq!(read_u32(&cooked.psau, 20).unwrap() as usize, samples.len());
        assert_eq!(cooked.report.cooked_sample_count as usize, samples.len());
    }

    #[test]
    fn one_shot_cook_caps_high_rate_sources_and_preserves_low_rate_sources() {
        // Equal-duration sources: 448 @ 44.1 kHz and 112 @ 11.025 kHz.
        let high = write_wav_mono_i16(44_100, &[0; 448]);
        let low = write_wav_mono_i16(11_025, &[0; 112]);

        let high = cook_sfx_from_wav(&high).unwrap();
        let low = cook_sfx_from_wav(&low).unwrap();

        assert_eq!(read_u32(&high, 16).unwrap(), 22_050);
        assert_eq!(read_u32(&low, 16).unwrap(), 11_025);
        assert!(low.len() < high.len());
    }

    #[test]
    fn cook_cdda_from_wav_emits_sector_padded_stereo_pcm() {
        let samples = [100i16, -100, 200, -200];
        let wav = write_wav_mono_i16(44_100, &samples);
        let cooked = cook_cdda_track_from_wav(&wav).unwrap();

        assert_eq!(cooked.len() % CDDA_SECTOR_BYTES, 0);
        assert!(cooked.len() >= CDDA_SECTOR_BYTES);
        for (i, sample) in samples.iter().enumerate() {
            let off = i * 4;
            assert_eq!(i16::from_le_bytes([cooked[off], cooked[off + 1]]), *sample);
            assert_eq!(
                i16::from_le_bytes([cooked[off + 2], cooked[off + 3]]),
                *sample
            );
        }
        assert!(cooked[samples.len() * 4..].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn cook_cdda_speed_multiplier_shortens_baked_track() {
        let samples: Vec<i16> = (0..44_100).map(|i| (i % 1024) as i16).collect();
        let wav = write_wav_mono_i16(44_100, &samples);
        let normal = cook_cdda_track_from_wav_at_speed(&wav, 4096).unwrap();
        let double = cook_cdda_track_from_wav_at_speed(&wav, 8192).unwrap();

        assert_eq!(normal.len() % CDDA_SECTOR_BYTES, 0);
        assert_eq!(double.len() % CDDA_SECTOR_BYTES, 0);
        assert!(double.len() < normal.len());
        assert!(double.len() >= normal.len() / 2);
    }

    #[test]
    fn adpcm_encoder_is_deterministic() {
        let samples: Vec<i16> = (0..512)
            .map(|i| (((i * 17) % 97) * 300 - 12_000) as i16)
            .collect();
        let wav = write_wav_mono_i16(22_050, &samples);
        let a = cook_sfx_from_wav(&wav).unwrap();
        let b = cook_sfx_from_wav(&wav).unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn spu_playback_renders_a_psau_at_the_console_rate() {
        let samples: Vec<i16> = (0..11_025)
            .map(|i| ((i as f64 * 0.3).sin() * 9_000.0) as i16)
            .collect();
        let psau = cook_sfx_from_wav(&write_wav_mono_i16(11_025, &samples)).unwrap();
        let wav = spu_playback_wav(&psau).unwrap();
        let parsed = parse_wav_pcm(&wav).unwrap();

        assert_eq!(parsed.sample_rate_hz, 44_100);
        // One second of source is one second of playback.
        assert!((parsed.samples.len() as i64 - 44_100).abs() <= 4);
        assert!(parsed.samples.iter().any(|&s| s.abs() > 20_000));
        assert!(spu_playback_wav(b"not audio at all, just some bytes and more bytes").is_err());
    }

    #[test]
    fn raw_spu_cook_resamples_and_marks_one_shot_blocks() {
        let samples: Vec<i16> = (0..112).map(|i| (i as i16 - 56) * 128).collect();
        let wav = write_wav_mono_i16(22_050, &samples);
        let cooked = cook_spu_adpcm_from_wav(&wav, 11_025, None).unwrap();

        assert_eq!(cooked.source_sample_rate_hz, 22_050);
        assert_eq!(cooked.source_channels, 1);
        assert_eq!(cooked.source_sample_count, 112);
        assert_eq!(cooked.sample_rate_hz, 11_025);
        assert_eq!(cooked.sample_count, 56);
        assert_eq!(cooked.loop_start, None);
        assert_eq!(cooked.adpcm.len(), ADPCM_BLOCK_BYTES * 2);
        assert_eq!(cooked.adpcm[1], 0x04);
        assert_eq!(cooked.adpcm[ADPCM_BLOCK_BYTES + 1], 0x01);
    }

    #[test]
    fn raw_spu_cook_marks_loop_start_and_looping_end() {
        let samples: Vec<i16> = (0..112).map(|i| (i as i16 - 56) * 128).collect();
        let wav = write_wav_mono_i16(22_050, &samples);
        let cooked = cook_spu_adpcm_from_wav(&wav, 11_025, Some(56)).unwrap();

        assert_eq!(cooked.loop_start, Some(28));
        assert_eq!(cooked.adpcm.len(), ADPCM_BLOCK_BYTES * 2);
        assert_eq!(cooked.adpcm[1], 0x00);
        // The loop block is also the last: LOOP_START and END|REPEAT together
        // (the previous encoder overwrote the loop start here).
        assert_eq!(cooked.adpcm[ADPCM_BLOCK_BYTES + 1], 0x07);
        assert_eq!(
            cooked.adpcm[ADPCM_BLOCK_BYTES] >> 4,
            0,
            "history-free re-entry"
        );
    }

    #[test]
    fn raw_spu_cook_uses_combined_flags_for_single_block() {
        let wav = write_wav_mono_i16(11_025, &[0; ADPCM_SAMPLES_PER_BLOCK]);

        let one_shot = cook_spu_adpcm_from_wav(&wav, 11_025, None).unwrap();
        let looping = cook_spu_adpcm_from_wav(&wav, 11_025, Some(0)).unwrap();

        assert_eq!(one_shot.adpcm[1], 0x05);
        assert_eq!(looping.adpcm[1], 0x07);
    }
}
