use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{BufRead, Read};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(C = "lowercase")]
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

static BY_EXTENSION: phf::Map<&'static str, &'static [&'static str]> = phf::phf_map! {
    "c" => &["c", "h"],
    "cpp" => &["cpp", "cxx", "cc", "hpp", "hxx", "hh"],
    "java" => &["java"],
    "js" => &["js"],
    "rs" => &["rs"],
    "ts" => &["ts"],
    "sh" => &["sh"],
    "bash" => &["bash"],
    "zsh" => &["zsh"],
    "fish" => &["fish"],
    "python" => &["py"],
    "ruby" => &["rb"],
    "perl" => &["pl", "pm"],
    "php" => &["php"],
    "html" => &["html", "htm"],
    "css" => &["css"],
    "xml" => &["xml"],
    "json" => &["json"],
    "yaml" => &["yaml", "yml"],
    "toml" => &["toml"],
    "ini" => &["ini"],
    "md" => &["md"],
    "tex" => &["tex"],
    "latex" => &["latex"],
    "sql" => &["sql"],
    "asm" => &["asm", "s"],
    "csharp" => &["cs"],
    "go" => &["go"],
    "haskell" => &["hs"],
    "lisp" => &["lisp"],
    "lua" => &["lua"],
    "ocaml" => &["ml"],
    "r" => &["r"],
    "scala" => &["scala"],
    "swift" => &["swift"],
    "vb" => &["vb"],
    "vbscript" => &["vbs"],
    "verilog" => &["v"],
    "vhdl" => &["vhd", "vhdl"],
    "make" => &["makefile", "Makefile", "make"],
    "cmake" => &["CMakeLists.txt", "cmake"],
    "dockerfile" => &["Dockerfile"],
    "plaintext" => &["txt"],
    "unknown" => &[""],
};

static BY_FILENAME: phf::Map<&'static str, &'static [&'static str]> = phf::phf_map! {
    "Makefile" => &["make"],
    "Dockerfile" => &["dockerfile"],
    "CMakeLists.txt" => &["cmake"],
};

static BY_INTERPRETER: phf::Map<&'static str, &'static [&'static str]> = phf::phf_map! {
    "bash" => &["sh"],
    "python" => &["py"],
    "ruby" => &["rb"],
    "perl" => &["pl", "pm"],
    "php" => &["php"],
    "node" => &["js"],
    "nodejs" => &["js"],
    "lua" => &["lua"],
    "sh" => &["sh"],
    "zsh" => &["zsh"],
    "fish" => &["fish"],
    "python2" => &["py"],
    "python3" => &["py"],
    "ruby2" => &["rb"],
    "ruby3" => &["rb"],
    "perl5" => &["pl", "pm"],
    "perl6" => &["pl", "pm"],
    "php5" => &["php"],
    "php7" => &["php"],
};

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
    } else if cfg!(unix) {
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

    if let Ok(from_filename) = tags_from_file_name(path.file_name().) {
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

fn tags_from_file_name(filename: String) -> Result<Vec<FileTag>> {

}

fn tags_from_interpreter(interpreter: &[String]) -> Vec<FileTag> {
    let mut tags = Vec::new();
    if interpreter.is_empty() {
        return tags;
    }

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
