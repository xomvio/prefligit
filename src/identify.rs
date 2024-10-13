use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Read};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileTag {
    Directory,
    Symlink,
    Socket,
    FIFO,
    BlockDevice,
    CharacterDevice,
    File,

    Executable,
    NonExecutable,

    Text,
    Binary,

    Other(Cow<'static, &'static str>),
}

fn by_extension() -> &'static HashMap<&'static str, Vec<&'static str>> {
    static EXTENSIONS: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    EXTENSIONS.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert("c", vec!["c", "h"]);
        map.insert("cpp", vec!["cpp", "cxx", "cc", "hpp", "hxx", "hh"]);
        map.insert("java", vec!["java"]);
        map.insert("js", vec!["js"]);
        map.insert("rs", vec!["rs"]);
        map.insert("ts", vec!["ts"]);
        map.insert("sh", vec!["sh"]);
        map.insert("bash", vec!["bash"]);
        map.insert("zsh", vec!["zsh"]);
        map.insert("fish", vec!["fish"]);
        map.insert("python", vec!["py"]);
        map.insert("ruby", vec!["rb"]);
        map.insert("perl", vec!["pl", "pm"]);
        map.insert("php", vec!["php"]);
        map.insert("html", vec!["html", "htm"]);
        map.insert("css", vec!["css"]);
        map.insert("xml", vec!["xml"]);
        map.insert("json", vec!["json"]);
        map.insert("yaml", vec!["yaml", "yml"]);
        map.insert("toml", vec!["toml"]);
        map.insert("ini", vec!["ini"]);
        map.insert("md", vec!["md"]);
        map.insert("tex", vec!["tex"]);
        map.insert("latex", vec!["latex"]);
        map.insert("sql", vec!["sql"]);
        map.insert("asm", vec!["asm", "s"]);
        map.insert("csharp", vec!["cs"]);
        map.insert("go", vec!["go"]);
        map.insert("haskell", vec!["hs"]);
        map.insert("lisp", vec!["lisp"]);
        map.insert("lua", vec!["lua"]);
        map.insert("ocaml", vec!["ml"]);
        map.insert("r", vec!["r"]);
        map.insert("scala", vec!["scala"]);
        map.insert("swift", vec!["swift"]);
        map.insert("vb", vec!["vb"]);
        map.insert("vbscript", vec!["vbs"]);
        map.insert("verilog", vec!["v"]);
        map.insert("vhdl", vec!["vhd", "vhdl"]);
        map
    })
}

fn by_filename() -> &'static HashMap<&'static str, Vec<&'static str>> {
    static FILENAMES: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    FILENAMES.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert("Makefile", vec!["make"]);
        map.insert("Dockerfile", vec!["dockerfile"]);
        map.insert("CMakeLists.txt", vec!["cmake"]);
        map
    })
}

fn by_interpreter() -> &'static HashMap<&'static str, Vec<&'static str>> {
    static INTERPRETERS: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    INTERPRETERS.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert("bash", vec!["sh"]);
        map.insert("python", vec!["py"]);
        map.insert("ruby", vec!["rb"]);
        map.insert("perl", vec!["pl", "pm"]);
        map.insert("php", vec!["php"]);
        map.insert("node", vec!["js"]);
        map.insert("nodejs", vec!["js"]);
        map.insert("lua", vec!["lua"]);
        map.insert("sh", vec!["sh"]);
        map.insert("zsh", vec!["zsh"]);
        map.insert("fish", vec!["fish"]);
        map.insert("python2", vec!["py"]);
        map.insert("python3", vec!["py"]);
        map.insert("ruby2", vec!["rb"]);
        map.insert("ruby3", vec!["rb"]);
        map.insert("perl5", vec!["pl", "pm"]);
        map.insert("perl6", vec!["pl", "pm"]);
        map.insert("php5", vec!["php"]);
        map.insert("php7", vec!["php"]);
        map
    })
}

impl FileTag {
    pub fn is_type_tag(&self) -> bool {
        match self {
            FileTag::Directory | FileTag::Symlink | FileTag::Socket | FileTag::File => true,
            _ => false,
        }
    }

    pub fn is_mode_tag(&self) -> bool {
        match self {
            FileTag::Executable | FileTag::NonExecutable => true,
            _ => false,
        }
    }

    pub fn is_encoding_tag(&self) -> bool {
        match self {
            FileTag::Text | FileTag::Binary => true,
            _ => false,
        }
    }
}

pub fn tags_from_path(path: &Path) -> Result<Vec<FileTag>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        return Ok(vec![FileTag::Directory]);
    } else if metadata.is_symlink() {
        return Ok(vec![FileTag::Symlink]);
    }
    #[cfg(unix)]
    {
        let file_type = metadata.file_type();
        if file_type.is_socket() {
            return Ok(vec![FileTag::Socket]);
        } else if file_type.is_fifo() {
            return Ok(vec![FileTag::FIFO]);
        } else if file_type.is_block_device() {
            return Ok(vec![FileTag::BlockDevice]);
        } else if file_type.is_char_device() {
            return Ok(vec![FileTag::CharacterDevice]);
        }
    };

    let mut tags = HashSet::new();
    tags.insert(FileTag::File);

    #[cfg(unix)]
    let executable = metadata.permissions().mode() & 0o111 != 0;
    #[cfg(not(unix))]
    let executable = {
        let ext = path.extension().and_then(|ext| ext.to_str());
        ext.map_or(false, |ext| ext == "exe" || ext == "bat" || ext == "cmd")
    };

    if executable {
        tags.insert(FileTag::Executable);
    } else {
        tags.insert(FileTag::NonExecutable);
    }

    // TODO: fix
    if let Ok(from_filename) = tags_from_file_name(String::new()) {
        tags.extend(from_filename);
    } else {
        if executable {
            if let Ok(shebang) = parse_shebang(path) {
                tags.extend(tags_from_interpreter(&shebang));
            }
        }
    }

    if !tags.iter().any(|tag| tag.is_encoding_tag()) {
        if file_is_text(path) {
            tags.insert(FileTag::Text);
        } else {
            tags.insert(FileTag::Binary);
        }
    }

    Ok(tags.into_iter().collect())
}

fn tags_from_file_name(_filename: String) -> Result<Vec<FileTag>> {
    todo!()
}

fn tags_from_interpreter(_interpreter: &[String]) -> Vec<FileTag> {
    todo!()
}

#[derive(thiserror::Error, Debug)]
enum ShebangError {
    #[error("No shebang found")]
    NoShebang,
    #[error("Shebang contains non-printable characters")]
    NonPrintableChars,
    #[error("Failed to parse shebang")]
    ParseFailed,
    #[error("No command found in shebang")]
    NoCommand,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

fn parse_shebang(path: &Path) -> Result<Vec<String>, ShebangError> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if !line.starts_with("#!") {
        return Err(ShebangError::NoShebang);
    }

    // Require only printable ASCII
    if line.bytes().any(|b| b < 0x20 || b > 0x7E) {
        return Err(ShebangError::NonPrintableChars);
    }

    let tokens = shlex::split(line[2..].trim()).ok_or(ShebangError::ParseFailed)?;
    let cmd = if tokens.starts_with(&[String::from("/usr/bin/env"), String::from("-S")]) {
        tokens[2..].to_vec()
    } else if tokens.starts_with(&[String::from("/usr/bin/env")]) {
        tokens[1..].to_vec()
    } else {
        tokens
    };
    if cmd.is_empty() {
        return Err(ShebangError::NoCommand);
    }
    if cmd[0] == "nix-shell" {
        return Ok(vec![]);
    }
    Ok(cmd)
}

/// Return whether the first KB of contents seems to be binary.
///
/// This is roughly based on libmagic's binary/text detection:
/// https://github.com/file/file/blob/df74b09b9027676088c797528edcaae5a9ce9ad0/src/encoding.c#L203-L228
fn file_is_text(path: &Path) -> bool {
    let mut buffer = [0; 1024];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };

    let Ok(bytes_read) = file.read(&mut buffer) else {
        return false;
    };
    if bytes_read == 0 {
        return true;
    }

    let text_chars: Vec<u8> = (0..=255)
        .filter(|&x| {
            (x >= 0x20 && x <= 0x7E) // Printable ASCII
                || x >= 0x80 // High bit set
                || [7, 8, 9, 10, 11, 12, 13, 27].contains(&x) // Control characters
        })
        .collect();

    buffer[..bytes_read]
        .iter()
        .all(|&b| text_chars.contains(&b))
}
