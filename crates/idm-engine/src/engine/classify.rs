/// 文件分类模块：按扩展名自动分文件夹

/// 文件类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Video,      // 视频
    Audio,      // 音频
    Documents,  // 文档
    Archives,   // 压缩包
    Images,     // 图片
    Programs,   // 程序/安装包
    Other,      // 其他
}

impl FileCategory {
    /// 返回子目录名
    pub fn dir_name(&self) -> &str {
        match self {
            FileCategory::Video => "视频",
            FileCategory::Audio => "音频",
            FileCategory::Documents => "文档",
            FileCategory::Archives => "压缩包",
            FileCategory::Images => "图片",
            FileCategory::Programs => "程序",
            FileCategory::Other => "其他",
        }
    }

    /// 返回英文目录名，用于兼容路径
    pub fn dir_name_en(&self) -> &str {
        match self {
            FileCategory::Video => "video",
            FileCategory::Audio => "music",
            FileCategory::Documents => "documents",
            FileCategory::Archives => "archives",
            FileCategory::Images => "images",
            FileCategory::Programs => "programs",
            FileCategory::Other => "other",
        }
    }
}

/// 根据文件名推断文件类别
pub fn classify_filename(filename: &str) -> FileCategory {
    let lower = filename.to_lowercase();

    // 视频
    if matches_any(&lower, &[
        ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm",
        ".m4v", ".mpg", ".mpeg", ".3gp", ".ogv", ".ts", ".m2ts",
    ]) {
        return FileCategory::Video;
    }

    // 音频
    if matches_any(&lower, &[
        ".mp3", ".wav", ".flac", ".aac", ".ogg", ".wma", ".m4a",
        ".opus", ".ape", ".alac", ".aiff", ".mid", ".midi",
    ]) {
        return FileCategory::Audio;
    }

    // 文档
    if matches_any(&lower, &[
        ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        ".txt", ".md", ".csv", ".rtf", ".odt", ".ods", ".odp",
        ".epub", ".mobi", ".html", ".htm", ".json", ".xml", ".yaml", ".yml",
        ".log", ".cfg", ".ini", ".conf",
    ]) {
        return FileCategory::Documents;
    }

    // 压缩包
    if matches_any(&lower, &[
        ".zip", ".rar", ".7z", ".tar", ".gz", ".xz", ".bz2", ".zst",
        ".iso", ".dmg", ".tgz", ".tbz2", ".txz", ".lz", ".lz4", ".arj",
        ".cab", ".deb", ".rpm",
    ]) {
        return FileCategory::Archives;
    }

    // 图片
    if matches_any(&lower, &[
        ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".svg", ".webp",
        ".ico", ".tiff", ".tif", ".psd", ".ai", ".eps", ".raw",
        ".heic", ".heif", ".avif",
    ]) {
        return FileCategory::Images;
    }

    // 程序/安装包
    if matches_any(&lower, &[
        ".exe", ".msi", ".dmg", ".apk", ".appx", ".bat", ".cmd",
        ".ps1", ".sh", ".app", ".run", ".bin",
    ]) {
        return FileCategory::Programs;
    }

    FileCategory::Other
}

fn matches_any(lower: &str, exts: &[&str]) -> bool {
    exts.iter().any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_video() {
        assert_eq!(classify_filename("movie.mp4"), FileCategory::Video);
        assert_eq!(classify_filename("video.MKV"), FileCategory::Video);
        assert_eq!(classify_filename("clip.webm"), FileCategory::Video);
    }

    #[test]
    fn test_classify_audio() {
        assert_eq!(classify_filename("song.mp3"), FileCategory::Audio);
        assert_eq!(classify_filename("podcast.FLAC"), FileCategory::Audio);
    }

    #[test]
    fn test_classify_documents() {
        assert_eq!(classify_filename("report.pdf"), FileCategory::Documents);
        assert_eq!(classify_filename("data.csv"), FileCategory::Documents);
        assert_eq!(classify_filename("readme.md"), FileCategory::Documents);
    }

    #[test]
    fn test_classify_archives() {
        assert_eq!(classify_filename("backup.zip"), FileCategory::Archives);
        assert_eq!(classify_filename("source.tar.gz"), FileCategory::Archives);
    }

    #[test]
    fn test_classify_programs() {
        assert_eq!(classify_filename("setup.exe"), FileCategory::Programs);
        assert_eq!(classify_filename("app.msi"), FileCategory::Programs);
    }

    #[test]
    fn test_classify_images() {
        assert_eq!(classify_filename("photo.jpg"), FileCategory::Images);
        assert_eq!(classify_filename("icon.PNG"), FileCategory::Images);
    }

    #[test]
    fn test_classify_other() {
        assert_eq!(classify_filename("unknown.xyz"), FileCategory::Other);
        assert_eq!(classify_filename("no_extension"), FileCategory::Other);
    }

    #[test]
    fn test_dir_names() {
        assert_eq!(FileCategory::Video.dir_name_en(), "video");
        assert_eq!(FileCategory::Audio.dir_name_en(), "music");
        assert_eq!(FileCategory::Documents.dir_name_en(), "documents");
    }
}
