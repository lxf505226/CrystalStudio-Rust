//! 素材层错误类型（手写 `Display` + `std::error::Error`，不引 thiserror）。

use std::fmt;
use std::io;
use std::path::PathBuf;

/// 图库素材层的失败原因。
///
/// 只覆盖「结构层面无法继续」的情况：扫描根不存在、目录读不了、一个库都没登记、单库打开失败。
/// **帧级问题（库不存在 / 帧号越界 / 空帧）不走这里**，
/// 按 [`crate::SpriteSource::frame`] 的契约返回 `None`。
#[derive(Debug)]
pub enum AssetError {
    /// 扫描根不存在，或存在但不是目录。
    RootNotDir {
        /// 传入的扫描根路径。
        path: PathBuf,
    },
    /// 文件系统访问失败（读目录 / 读文件）。
    Io {
        /// 出错的路径。
        path: PathBuf,
        /// 底层 IO 错误。
        source: io::Error,
    },
    /// 扫描完成但一个可登记的库都没有。
    NoLibraries {
        /// 扫描根路径。
        root: PathBuf,
    },
    /// 单个库文件打开或解析失败。
    ///
    /// `wil2atlas` 的解码器返回 `anyhow::Result`；本 crate 的依赖白名单不允许引入
    /// `anyhow`，故在此把错误链收成一段字符串（保留完整 `{:#}` 上下文）。
    OpenLibFailed {
        /// 库数据文件路径。
        path: PathBuf,
        /// 解码器给出的原因（已含 `anyhow` 上下文链）。
        reason: String,
    },
}

impl AssetError {
    /// 构造「IO 失败」错误（把路径与底层错误一起带上）。
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> AssetError {
        AssetError::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::RootNotDir { path } => {
                write!(f, "扫描根不存在或不是目录：{}", path.display())
            }
            AssetError::Io { path, source } => {
                write!(f, "文件访问失败：{}（{source}）", path.display())
            }
            AssetError::NoLibraries { root } => write!(
                f,
                "在 {} 下没有登记到任何图库（要求同名数据+索引成对：.wzl+.wzx 或 .wil+.wix）",
                root.display()
            ),
            AssetError::OpenLibFailed { path, reason } => {
                write!(f, "打开图库失败：{}（{reason}）", path.display())
            }
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AssetError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 错误信息含路径与原因() {
        let e = AssetError::RootNotDir {
            path: PathBuf::from(r"E:\nope"),
        };
        let msg = e.to_string();
        assert!(msg.contains(r"E:\nope"), "{msg}");
        assert!(msg.contains("目录"), "{msg}");
    }

    #[test]
    fn 无库错误说清成对要求() {
        let msg = AssetError::NoLibraries {
            root: PathBuf::from(r"E:\empty"),
        }
        .to_string();
        assert!(msg.contains("没有登记到任何图库"), "{msg}");
        assert!(msg.contains(".wzl+.wzx"), "{msg}");
        assert!(msg.contains(".wil+.wix"), "{msg}");
    }

    #[test]
    fn 可当标准错误装箱且能取到source() {
        use std::error::Error;

        let inner = io::Error::new(io::ErrorKind::NotFound, "缺文件");
        let e = AssetError::io("x.wzl", inner);
        assert!(e.source().is_some(), "Io 变体应提供 source()");

        let boxed: Box<dyn Error> = Box::new(AssetError::NoLibraries {
            root: PathBuf::from("r"),
        });
        assert!(boxed.to_string().contains("没有登记到任何图库"));
    }
}
