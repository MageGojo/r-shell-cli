//! GUI SFTP 文件管理 API —— 复用 r-shell-core 的 SFTP 能力。
//!
//! 列目录走结构化的 [`SftpEntryDto`];上传 / 下载用 [`StreamSink`] 把进度
//! ([`TransferProgress`]) 单向推给 Dart(与终端 PTY 同范式)。所有调用先
//! [`ensure_session`](super::ensure_session) 复用终端 / 文件管理器共享的活跃 SSH 会话。
//!
//! 数值字段(size / mtime / 进度)用 `i64` 干净映射 Dart `int`;**体积与时间的
//! 展示格式化交给 Dart 侧**。凭据只在 Rust 侧用,绝不回传。

use r_shell_core::native_backend::SftpEntry;

use crate::frb_generated::StreamSink;

/// 单条远程文件 / 目录(脱敏:仅元数据,无内容)。
pub struct SftpEntryDto {
    pub name: String,
    /// "dir" | "file" | "symlink"
    pub kind: String,
    /// 字节大小(目录 / 未知为 0)。
    pub size: i64,
    /// Unix 权限串,如 `rwxr-xr-x`(未知为空)。
    pub permissions: String,
    /// 修改时间(Unix 秒;服务器未给为 0)。Dart 侧格式化。
    pub modified_unix: i64,
}

impl From<SftpEntry> for SftpEntryDto {
    fn from(entry: SftpEntry) -> Self {
        let kind = if entry.is_symlink {
            "symlink"
        } else if entry.is_dir {
            "dir"
        } else {
            "file"
        };
        Self {
            name: entry.name,
            kind: kind.to_string(),
            size: entry.size as i64,
            permissions: entry.permissions,
            modified_unix: entry.modified_unix,
        }
    }
}

/// 一帧传输进度。完成由 stream 自然结束(onDone)、失败由 stream 错误(onError)表达。
pub struct TransferProgress {
    pub transferred: i64,
    pub total: i64,
}

/// 列出某连接的远程目录(结构化,跨平台)。
pub async fn sftp_list(connection_id: String, path: String) -> Result<Vec<SftpEntryDto>, String> {
    super::ensure_session(&connection_id).await?;
    let entries = super::manager()
        .sftp_list(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(SftpEntryDto::from).collect())
}

/// 取远程默认目录(home)的绝对路径(`realpath(".")`)。
pub async fn sftp_home(connection_id: String) -> Result<String, String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .sftp_realpath(&connection_id, ".")
        .await
        .map_err(|e| e.to_string())
}

/// 把远程路径规范化为绝对形式(用于「返回上级」解析 `..`)。
pub async fn sftp_realpath(connection_id: String, path: String) -> Result<String, String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .sftp_realpath(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())
}

/// 递归删除远程文件 / 目录。
pub async fn sftp_delete(connection_id: String, path: String) -> Result<(), String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .sftp_delete(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())
}

/// 重命名 / 移动远程路径(`from` → `to`,均为绝对路径)。
pub async fn sftp_rename(connection_id: String, from: String, to: String) -> Result<(), String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .sftp_rename(&connection_id, &from, &to)
        .await
        .map_err(|e| e.to_string())
}

/// 读取远程文件全部字节(用于「打开编辑」)。注意大文件会整体进内存。
pub async fn sftp_read_file(connection_id: String, path: String) -> Result<Vec<u8>, String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .read_file_to_memory(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())
}

/// 把字节写回远程文件(用于「编辑后保存」,创建 / 覆盖)。
pub async fn sftp_write_file(
    connection_id: String,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    super::ensure_session(&connection_id).await?;
    super::manager()
        .write_file_from_bytes(&connection_id, &path, &data)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 上传本地文件到远程。进度经 `sink` 推送(Dart 得到 `Stream<TransferProgress>`)。
/// 进度在此节流(≥128KB 或完成才推一帧),避免大文件刷爆 stream。
pub async fn sftp_upload(
    connection_id: String,
    local_path: String,
    remote_path: String,
    sink: StreamSink<TransferProgress>,
) -> Result<(), String> {
    super::ensure_session(&connection_id).await?;
    let mut last_emitted = 0i64;
    super::manager()
        .sftp_upload(&connection_id, &local_path, &remote_path, |transferred, total| {
            let transferred = transferred as i64;
            let total = total as i64;
            if transferred == total || transferred - last_emitted >= 128 * 1024 {
                last_emitted = transferred;
                let _ = sink.add(TransferProgress { transferred, total });
            }
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 下载远程文件到本地。进度经 `sink` 推送(同上,带节流)。
pub async fn sftp_download(
    connection_id: String,
    remote_path: String,
    local_path: String,
    sink: StreamSink<TransferProgress>,
) -> Result<(), String> {
    super::ensure_session(&connection_id).await?;
    let mut last_emitted = 0i64;
    super::manager()
        .sftp_download(&connection_id, &remote_path, &local_path, |transferred, total| {
            let transferred = transferred as i64;
            let total = total as i64;
            if transferred == total || transferred - last_emitted >= 128 * 1024 {
                last_emitted = transferred;
                let _ = sink.add(TransferProgress { transferred, total });
            }
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
