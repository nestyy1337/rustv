use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;
use std::path::Path;
use std::{fs, path::PathBuf, process::Stdio, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use tokio::{io::AsyncBufReadExt, process::Command};

use crate::{
    models::movie::{Movie, MovieState},
    services::{
        movie_manager::StreamableVideo,
        torrent::{ActiveProcessing, ProcessingStatus},
    },
    shared::error::{
        ConverterError, Error, FFmpegSnafu, IoSnafu, MovieMissingReason, MovieNotFoundSnafu,
        NotAvailableForConversionSnafu, TokioIoSnafu, UnsupportedCodecSnafu,
        UnsupportedFormatSnafu,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    VP9,
    AV1,
    HEVC,
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let codec_str = match self {
            VideoCodec::H264 => "h264",
            VideoCodec::VP9 => "vp9",
            VideoCodec::AV1 => "av1",
            VideoCodec::HEVC => "hevc",
        };
        write!(f, "{}", codec_str)
    }
}

impl FromStr for VideoCodec {
    type Err = ConverterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hevc" => Ok(Self::HEVC),
            "h264" => Ok(Self::H264),
            "av1" => Ok(Self::AV1),
            _ => Err(UnsupportedCodecSnafu {
                codec: s.to_string(),
            }
            .build()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HLS {
    segment_duration: u32,
}

impl Default for HLS {
    fn default() -> Self {
        HLS {
            segment_duration: 10,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MP4 {
    pub bitrate: u32,
}

impl Streamable for MP4 {
    fn format(&self) -> StreamableVideoFormat {
        StreamableVideoFormat::MP4(self.clone())
    }
}

pub trait Streamable {
    fn format(&self) -> StreamableVideoFormat;
}

impl Streamable for HLS {
    fn format(&self) -> StreamableVideoFormat {
        StreamableVideoFormat::HLS(self.clone())
    }
}

#[derive(Debug, Clone)]
pub enum StreamableVideoFormat {
    HLS(HLS),
    MP4(MP4),
}

impl std::fmt::Display for StreamableVideoFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format_str = match self {
            StreamableVideoFormat::HLS(_) => "HLS",
            StreamableVideoFormat::MP4(_) => "MP4",
        };
        write!(f, "{}", format_str)
    }
}

impl StreamableVideoFormat {
    pub fn try_from_movie_path(movie_path: &Path) -> Result<Vec<Self>, Error> {
        let mut formats = Vec::new();

        let hls_path = movie_path.join("index.m3u8");
        if hls_path.exists() {
            formats.push(StreamableVideoFormat::HLS(HLS {
                segment_duration: 10,
            }));
        }

        let mp4_path = movie_path.join("video.mp4");
        if mp4_path.exists() {
            formats.push(StreamableVideoFormat::MP4(MP4 { bitrate: 5000 }));
        }

        if formats.is_empty() {
            return Err(NotAvailableForConversionSnafu {}.build().into());
        }

        Ok(formats)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RawVideoFormat {
    MKV,
    AVI,
    MP4,
    MOV,
    WMV,
    FLV,
}

impl RawVideoFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mkv" => Some(RawVideoFormat::MKV),
            "avi" => Some(RawVideoFormat::AVI),
            "mov" => Some(RawVideoFormat::MOV),
            "wmv" => Some(RawVideoFormat::WMV),
            "flv" => Some(RawVideoFormat::FLV),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum StreamableFormat {
    HLS,
    MP4,
}

impl std::fmt::Display for StreamableFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format_str = match self {
            StreamableFormat::HLS => "HLS",
            StreamableFormat::MP4 => "MP4",
        };
        write!(f, "{}", format_str)
    }
}

#[async_trait]
pub trait Converter: Sync {
    type Error;
    async fn convert(
        &self,
        input: &VideoFile,
        convert_to: StreamableFormat,
        movie: Movie,
        processing_queue: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
    ) -> Result<ConvertedVideo, Self::Error>;
    fn supports(&self, input_codec: &VideoCodec, output_format: &StreamableFormat) -> bool;
}

#[derive(Debug)]
pub struct VideoFile {
    pub codec: VideoCodec,
    pub video_name: String,
    pub path: PathBuf,
}

pub fn try_get_codec_from_path(path: &PathBuf) -> Result<VideoCodec, Error> {
    let regex = Regex::new("codec_name=([^\n]*?)\\n").unwrap();

    let raw_output = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg(path)
        .output()
        .map_err(|_| {
            FFmpegSnafu {
                operation: "getting video codec",
            }
            .build()
        })?
        .stdout;
    let raw_str = String::from_utf8(raw_output).unwrap_or_default();

    if let Some(codec_str) = regex.captures(&raw_str) {
        let codec = VideoCodec::from_str(&codec_str[1])?;
        Ok(codec)
    } else {
        Err(UnsupportedCodecSnafu {
            codec: "unknown".to_string(),
        }
        .build()
        .into())
    }
}

impl TryFrom<PathBuf> for VideoFile {
    type Error = Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let codec = try_get_codec_from_path(&path)?;
        Ok(Self {
            codec,
            video_name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .ok_or(
                    UnsupportedFormatSnafu {
                        format: path.to_string_lossy().to_string(),
                    }
                    .build(),
                )?,
            path,
        })
    }
}

impl TryFrom<&PathBuf> for VideoFile {
    type Error = Error;
    fn try_from(path: &PathBuf) -> Result<Self, Self::Error> {
        let regex = Regex::new("codec_name=([^\n]*?)\\n").unwrap();

        let raw_output = std::process::Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=codec_name")
            .arg(path)
            .output()
            .map_err(|_| {
                FFmpegSnafu {
                    operation: "getting video codec",
                }
                .build()
            })?
            .stdout;
        let raw_str = String::from_utf8(raw_output).unwrap_or_default();

        let codec: VideoCodec;
        if let Some(codec_str) = regex.captures(&raw_str) {
            codec = VideoCodec::from_str(&codec_str[1])?;
        } else {
            return Err(UnsupportedCodecSnafu {
                codec: "unknown".to_string(),
            }
            .build()
            .into());
        }

        Ok(Self {
            codec,
            video_name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .ok_or(
                    UnsupportedFormatSnafu {
                        format: path.to_string_lossy().to_string(),
                    }
                    .build(),
                )?,
            path: path.to_path_buf(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ConvertedVideo {
    pub path: PathBuf,
    pub formats: Vec<StreamableVideoFormat>,
    pub movie: Movie,
}

#[derive(Clone, Debug)]
pub struct FFmpegConverter;

#[async_trait]
impl Converter for FFmpegConverter {
    type Error = Error;
    #[tracing::instrument(name = "video conversion", skip(self, input, convert_to))]
    async fn convert(
        &self,
        input: &VideoFile,
        convert_to: StreamableFormat,
        movie: Movie,
        processing_queue: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
    ) -> Result<ConvertedVideo, Self::Error> {
        let output_path = convert(input, &movie, &convert_to, processing_queue).await?;
        tracing::info!(output_path = ?output_path, "Converted video saved");

        if let StreamableFormat::HLS = convert_to {
            let m3u8_path = output_path.join(&input.video_name).with_extension("m3u8");
            let index_path = m3u8_path
                .parent()
                .ok_or(
                    MovieNotFoundSnafu {
                        movie_id: movie.id,
                        reason: MovieMissingReason::NoFile,
                    }
                    .build(),
                )?
                .join("index.m3u8");

            if !index_path.exists() {
                fs::rename(&m3u8_path, &index_path).context(IoSnafu {
                    operation: "renaming m3u8 file",
                })?;
            }
            tracing::info!(index_path = ?index_path, "Adjusted m3u8 file");
        }

        let hls = HLS::default();

        tracing::info!(
            movie_id = movie.id,
            conversion_format = ?convert_to.to_string(),
            "Video conversion completed"
        );

        Ok(ConvertedVideo {
            path: output_path,
            formats: vec![StreamableVideoFormat::HLS(hls)],
            movie,
        })
    }

    fn supports(&self, input_codec: &VideoCodec, output_format: &StreamableFormat) -> bool {
        match input_codec {
            VideoCodec::HEVC => match output_format {
                StreamableFormat::HLS => true,
                StreamableFormat::MP4 => true,
            },
            VideoCodec::H264 => match output_format {
                StreamableFormat::HLS => true,
                StreamableFormat::MP4 => true,
            },
            VideoCodec::VP9 => match output_format {
                StreamableFormat::MP4 => true,
                StreamableFormat::HLS => false,
            },
            VideoCodec::AV1 => match output_format {
                StreamableFormat::MP4 => true,
                StreamableFormat::HLS => false,
            },
        }
    }
}

impl VideoFile {
    pub fn parent(&self) -> Option<PathBuf> {
        self.path.parent().map(|parent| parent.to_path_buf())
    }
}

async fn convert(
    input: &VideoFile,
    movie: &Movie,
    _output_format: &StreamableFormat,
    processing_queue: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
) -> Result<PathBuf, Error> {
    let base = input.parent().ok_or(
        MovieNotFoundSnafu {
            movie_id: 0,
            reason: MovieMissingReason::NoFile,
        }
        .build(),
    )?;

    tracing::info!(base = ?base, "Creating output directory");
    fs::create_dir_all(&base).context(IoSnafu {
        operation: "creating output directory",
    })?;

    let output_m3u8 = base.join("index.m3u8");
    let segment_pattern = base.join("%05d.ts");

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-init_hw_device",
            "vaapi=amd:/dev/dri/renderD128",
            "-filter_hw_device",
            "amd",
            "-hwaccel",
            "vaapi",
            "-hwaccel_output_format",
            "vaapi",
            "-i",
        ])
        .arg(&input.path)
        .args(["-t", "120"])
        .args([
            "-vf",
            "scale_vaapi=format=nv12",
            "-c:v",
            "h264_vaapi",
            "-b:v",
            "5M",
            "-maxrate",
            "5M",
            "-bufsize",
            "10M",
            "-rc_mode",
            "CBR",
            "-g",
            "240",
            "-keyint_min",
            "240",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-f",
            "hls",
            "-hls_time",
            "10",
            "-hls_playlist_type",
            "vod",
            "-hls_list_size",
            "0",
            "-hls_flags",
            "independent_segments",
            "-hls_segment_filename",
        ])
        .arg(segment_pattern)
        .arg(output_m3u8)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            FFmpegSnafu {
                operation: "executing ffmpeg vaapi",
            }
            .build()
        })?;

    let stderr = child.stderr.take().ok_or(
        FFmpegSnafu {
            operation: "taking ffmpeg stderr",
        }
        .build(),
    )?;
    let mut reader = tokio::io::BufReader::new(stderr).lines();
    // we need select! macro since there is no way to handle both reading stderr without blocking
    // and waiting for process to end, so we periodically read stderr and check if process ended
    // so that we can leave the bufreader loop.
    // alternative would be to spawn a separate task for reading stderr, but that is against the
    // idea of structured concurrency.
    // passing the child-handle through a channel is an alternative and would probably be cleaner
    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                while let Some(line) = reader.next_line().await.context(TokioIoSnafu {
                operation: "reading ffmpeg stderr",
            })? {
                    let status = ProcessingStatus::from_stderr(&line);
                    if let Some(status) = status {
                        let write = ActiveProcessing { status };
                        processing_queue.write().await.insert(movie.title.clone(), write);
                    }
                }
            }
            status = child.wait() => {
                let status = status.context(TokioIoSnafu {
                    operation: "waiting for ffmpeg process",
                })?;
                if !status.success() {
                    return Err(FFmpegSnafu {
                        operation: "ffmpeg process failed",
                    }
                    .build());
                }
                break;
            }
        }
    }
    Ok(base.to_path_buf())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_processing_status_from_stderr() {
        let stderr = "frame=  240 fps=0.0 q=-1.0 size=    1024kB time=00:00:10.00 bitrate= 838.9kbits/s speed=1.00x";
        let status = ProcessingStatus::from_stderr(stderr).unwrap();
        assert_eq!(status.elapsed, "00:00:10.00");
        assert_eq!(status.speed, "1.00x");
        assert_eq!(status.progress, "240");
    }
    #[test]
    fn test_processing_status_from_stderr_missing_fields() {
        let stderr = "frame=  240 fps=0.0 q=-1.0 size=    1024kB bitrate= 838.9kbits/s speed=1.00x";
        let status = ProcessingStatus::from_stderr(stderr);
        assert!(status.is_none());
    }

    #[test]
    fn test_processing_status_missalligned_frame() {
        let stderr = "frame= fps=0.0 240  q=-1.0 size=    1024kB time=00:00:10.00 bitrate= 838.9kbits/s speed=1.00x";
        let status = ProcessingStatus::from_stderr(stderr);
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.elapsed, "00:00:10.00");
        assert_eq!(status.speed, "1.00x");
        assert_eq!(status.progress, "fps=0.0");
    }

    #[test]
    fn test_processing_status_empty() {
        let stderr = "";
        let status = ProcessingStatus::from_stderr(stderr);
        assert!(status.is_none());
    }

    #[test]
    fn test_video_codec_from_str_valid() {
        assert_eq!(VideoCodec::from_str("h264").unwrap(), VideoCodec::H264);
        assert_eq!(VideoCodec::from_str("hevc").unwrap(), VideoCodec::HEVC);
        assert_eq!(VideoCodec::from_str("av1").unwrap(), VideoCodec::AV1);
    }

    #[test]
    fn test_video_codec_from_str_invalid() {
        let result = VideoCodec::from_str("vp9");
        assert!(result.is_err());

        let result = VideoCodec::from_str("unknown");
        assert!(result.is_err());

        let result = VideoCodec::from_str("");
        assert!(result.is_err());
    }

    #[test]
    fn test_video_codec_to_string() {
        assert_eq!(VideoCodec::H264.to_string(), "h264");
        assert_eq!(VideoCodec::VP9.to_string(), "vp9");
        assert_eq!(VideoCodec::AV1.to_string(), "av1");
        assert_eq!(VideoCodec::HEVC.to_string(), "hevc");
    }

    #[test]
    fn test_raw_video_format_from_extension_valid() {
        assert_eq!(
            RawVideoFormat::from_extension("mkv"),
            Some(RawVideoFormat::MKV)
        );
        assert_eq!(
            RawVideoFormat::from_extension("avi"),
            Some(RawVideoFormat::AVI)
        );
        assert_eq!(
            RawVideoFormat::from_extension("mov"),
            Some(RawVideoFormat::MOV)
        );
        assert_eq!(
            RawVideoFormat::from_extension("wmv"),
            Some(RawVideoFormat::WMV)
        );
        assert_eq!(
            RawVideoFormat::from_extension("flv"),
            Some(RawVideoFormat::FLV)
        );
    }

    #[test]
    fn test_raw_video_format_from_extension_case_insensitive() {
        assert_eq!(
            RawVideoFormat::from_extension("MKV"),
            Some(RawVideoFormat::MKV)
        );
        assert_eq!(
            RawVideoFormat::from_extension("AVI"),
            Some(RawVideoFormat::AVI)
        );
        assert_eq!(
            RawVideoFormat::from_extension("MoV"),
            Some(RawVideoFormat::MOV)
        );
    }

    #[test]
    fn test_raw_video_format_from_extension_invalid() {
        assert_eq!(RawVideoFormat::from_extension("mp4"), None);
        assert_eq!(RawVideoFormat::from_extension("txt"), None);
        assert_eq!(RawVideoFormat::from_extension(""), None);
        assert_eq!(RawVideoFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_streamable_format_to_string() {
        assert_eq!(StreamableFormat::HLS.to_string(), "HLS");
        assert_eq!(StreamableFormat::MP4.to_string(), "MP4");
    }

    #[test]
    fn test_streamable_video_format_to_string() {
        let hls = StreamableVideoFormat::HLS(HLS::default());
        assert_eq!(hls.to_string(), "HLS");

        let mp4 = StreamableVideoFormat::MP4(MP4::default());
        assert_eq!(mp4.to_string(), "MP4");
    }

    #[test]
    fn test_ffmpeg_converter_supports_hevc() {
        let converter = FFmpegConverter;

        assert!(converter.supports(&VideoCodec::HEVC, &StreamableFormat::HLS));
        assert!(converter.supports(&VideoCodec::HEVC, &StreamableFormat::MP4));
    }

    #[test]
    fn test_ffmpeg_converter_supports_h264() {
        let converter = FFmpegConverter;

        assert!(converter.supports(&VideoCodec::H264, &StreamableFormat::HLS));
        assert!(converter.supports(&VideoCodec::H264, &StreamableFormat::MP4));
    }

    #[test]
    fn test_ffmpeg_converter_supports_vp9() {
        let converter = FFmpegConverter;

        assert!(!converter.supports(&VideoCodec::VP9, &StreamableFormat::HLS));
        assert!(converter.supports(&VideoCodec::VP9, &StreamableFormat::MP4));
    }

    #[test]
    fn test_ffmpeg_converter_supports_av1() {
        let converter = FFmpegConverter;

        assert!(!converter.supports(&VideoCodec::AV1, &StreamableFormat::HLS));
        assert!(converter.supports(&VideoCodec::AV1, &StreamableFormat::MP4));
    }

    #[test]
    fn test_hls_default() {
        let hls = HLS::default();
        assert_eq!(hls.segment_duration, 10);
    }

    #[test]
    fn test_mp4_default() {
        let mp4 = MP4::default();
        assert_eq!(mp4.bitrate, 0);
    }

    #[test]
    fn test_streamable_trait_hls() {
        let hls = HLS::default();
        let format = hls.format();

        match format {
            StreamableVideoFormat::HLS(_) => {}
            _ => panic!("Expected HLS format"),
        }
    }

    #[test]
    fn test_streamable_trait_mp4() {
        let mp4 = MP4 { bitrate: 5000 };
        let format = mp4.format();

        match format {
            StreamableVideoFormat::MP4(mp4_format) => {
                assert_eq!(mp4_format.bitrate, 5000);
            }
            _ => panic!("Expected MP4 format"),
        }
    }
}
