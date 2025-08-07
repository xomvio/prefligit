use anyhow::Result;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};

use crate::hook::Hook;
use crate::run::CONCURRENCY;

pub(crate) async fn fix_end_of_file(_hook: &Hook, filenames: &[&String]) -> Result<(i32, Vec<u8>)> {
    let mut tasks = futures::stream::iter(filenames)
        .map(async |filename| fix_file(filename).await)
        .buffered(*CONCURRENCY);

    let mut code = 0;
    let mut output = Vec::new();

    while let Some(result) = tasks.next().await {
        let (c, o) = result?;
        code |= c;
        output.extend(o);
    }

    Ok((code, output))
}

enum LineEnding {
    Crlf,
    Lf,
    Cr,
}

async fn empty_file(filename: &str) -> Result<(i32, Vec<u8>)> {
    // Make the file empty
    fs_err::tokio::remove_file(filename).await?;
    fs_err::tokio::File::create(filename).await?;
    Ok((0, Vec::new()))
}

struct ReverseReader {
    reader: BufReader<fs_err::tokio::File>,
    pos: u64,
}
impl ReverseReader {
    async fn new(filename: &str) -> Result<Self> {
        let file = fs_err::tokio::File::open(filename).await?;
        let reader = BufReader::new(file);
        let file_len = reader.get_ref().metadata().await?.len();
        if file_len == 0 {
            // File is empty
            return Ok(Self { reader, pos: 0 });
        }

        let pos = file_len - 1; // Start from the last byte
        Ok(Self { reader, pos })
    }
    async fn read_backwards(&mut self) -> Result<u8> {
        self.reader.seek(std::io::SeekFrom::Start(self.pos)).await?;
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf).await?;
        if self.pos == 0 {
            return Ok(0); // End(Beginning) of file reached
        }
        self.pos -= 1; // Move to the previous byte
        Ok(buf[0])
    }
}

async fn fix_file(filename: &str) -> Result<(i32, Vec<u8>)> {
    let src_file = fs_err::tokio::File::open(filename).await?;
    let file_len = src_file.metadata().await?.len();

    // If the file is empty, do nothing.
    if file_len == 0 {
        return Ok((0, Vec::new()));
    }
    // If the file is not empty, we will read it backwards to find the last byte
    let mut reader = ReverseReader::new(filename).await?;

    // Read the last byte of the file
    let c = reader.read_backwards().await?;

    let (mut newlines, line_ending) = if c == b'\n' {
        // Last character is LF
        // Read the previous character to see if it was newline character
        match reader.read_backwards().await {
            Ok(0) => {
                // No previous character, there is only one LF in the file
                // We will make it empty
                empty_file(filename).await?;
                return Ok((1, format!("Fixing {filename}\n").into_bytes()));
            }
            Ok(c) => {
                if c == b'\r' {
                    // File ends with CRLF
                    // We will see if we have more CRLFs at the end
                    (1, LineEnding::Crlf)
                } else if c == b'\n' {
                    // Previous character is LF, so we have two LFs
                    // We will see if we have more LFs at the end
                    (2, LineEnding::Lf)
                } else {
                    // File ends with one LF
                    // Nothing to fix
                    return Ok((0, Vec::new()));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Error reading file: {}", e));
            }
        }
    } else if c == b'\r' {
        // File ends with CR
        // We will see if we have multiple CRs at the end
        (1, LineEnding::Cr)
    } else {
        // File does not end with a newline
        // Read the whole file until find any line ending
        loop {
            match reader.read_backwards().await {
                // If file has no newline at all
                // Default to CRLF
                Ok(0) => break (0, LineEnding::Crlf),

                // See if we have a newline character
                Ok(c) => {
                    if c == b'\r' {
                        // Newline character is CR
                        break (0, LineEnding::Cr);
                    } else if c == b'\n' {
                        // We found a \n, see if the previous character is \r
                        match reader.read_backwards().await {
                            Ok(0) => {
                                // No previous character, so we have only one LF
                                // This is an edge case where we have one LF in the first byte of the file
                                break (0, LineEnding::Lf);
                            }
                            Ok(c) => {
                                if c == b'\r' {
                                    // Newline character is CRLF
                                    break (0, LineEnding::Crlf);
                                }
                                // Newline character is LF
                                break (0, LineEnding::Lf);
                            }
                            Err(e) => {
                                return Err(anyhow::anyhow!("Error reading file: {}", e));
                            }
                        }
                    }
                    // else
                    // No newline character found, continue reading backwards
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Error reading file: {}", e));
                }
            }
        }
    };

    // Count newlines at the end of the file
    if newlines != 0 {
        // We have at least one newline character at the end of the file
        // Check if we have multiple newlines at the end
        loop {
            match reader.read_backwards().await {
                Ok(0) => {
                    // End of file reached
                    // Whole file is newlines
                    empty_file(filename).await?;
                    return Ok((1, format!("Fixing {filename}\n").into_bytes()));
                }
                Ok(c) => {
                    match line_ending {
                        LineEnding::Lf => {
                            if c == b'\n' {
                                newlines += 1; // Count LF
                            } else {
                                // If we read something else, we stop
                                break;
                            }
                        }
                        LineEnding::Crlf => {
                            if c == b'\n' {
                                // If we read LF, check if the previous character was CR
                                match reader.read_backwards().await {
                                    Ok(0) => {
                                        // This is a very edge case.
                                        // No previous character, this means we have an LF without CR at the beginning of file
                                        // However, since we reached the beginning of the file, this still means all the file is newlines
                                        // We will make it empty
                                        empty_file(filename).await?;
                                        return Ok((
                                            1,
                                            format!("Fixing {filename}\n").into_bytes(),
                                        ));
                                    }
                                    Ok(c) => {
                                        if c == b'\r' {
                                            newlines += 1; // Count CRLF
                                        } else {
                                            // We have an LF without CR, this is unexpected in a CRLF(?) file.
                                            // However, last newline was CRLF, so we stop counting
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        return Err(anyhow::anyhow!("Error reading file: {}", e));
                                    }
                                }
                            }
                            /*else if c == b'\r' {
                                // Already counted CRLF, so we don't count it again
                            }*/
                            else {
                                // If we read something else, we stop
                                break;
                            }
                        }
                        LineEnding::Cr => {
                            if c == b'\r' {
                                newlines += 1; // Count CR
                            } else {
                                // If we read something else, we stop
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Error reading file: {}", e));
                }
            }
        }
    }
    // At this point, we have the number of newlines at the end of the file
    // It's time to fix the newlines at the end of the file
    // If we have more than one newline, we will keep only one

    // Calculate the new content length
    let new_file_len = match newlines.cmp(&1) {
        std::cmp::Ordering::Equal => {
            // We have only one newline at the end, nothing to fix
            return Ok((0, Vec::new()));
        }
        std::cmp::Ordering::Greater => {
            // We have more than one newline, should be only one
            match line_ending {
                LineEnding::Crlf => file_len - (newlines - 1) * 2, // CRLF is 2 bytes. So we remove 2 bytes for each
                LineEnding::Cr | LineEnding::Lf => file_len - (newlines - 1),
            }
        }
        std::cmp::Ordering::Less => {
            // We have no newlines at the end, we will add one
            match line_ending {
                LineEnding::Crlf => file_len + 2, // Add CRLF byte length
                LineEnding::Cr | LineEnding::Lf => file_len + 1, // Add CR or LF byte length
            }
        }
    };

    // re-define the file reader to start from the beginning
    let mut reader = BufReader::new(fs_err::tokio::File::open(filename).await?);

    // Define Buffered Writer to write to a temporary file
    let mut writer = BufWriter::new(fs_err::tokio::File::create(format!("{filename}.tmp")).await?);

    let mut buf = [0u8; 1]; // Buffer to read one byte at a time

    match new_file_len.cmp(&file_len) {
        std::cmp::Ordering::Less => {
            // We will truncate the file
            // Read only the necessary bytes and write them to the temporary file
            for _ in 0..new_file_len {
                reader.read_exact(&mut buf).await?;
                writer.write_all(&buf).await?;
            }
        }
        std::cmp::Ordering::Equal => {
            // The file is already the correct length, no need to fix
            return Ok((0, Vec::new()));
        }
        std::cmp::Ordering::Greater => {
            // We will extend the file
            // Read the whole file and write it to the temporary file
            for _ in 0..file_len {
                reader.read_exact(&mut buf).await?;
                writer.write_all(&buf).await?;
            }

            // Now we need to add the newline at the end
            match line_ending {
                LineEnding::Crlf => {
                    writer.write_all(b"\r\n").await?;
                }
                LineEnding::Lf => {
                    writer.write_all(b"\n").await?;
                }
                LineEnding::Cr => {
                    writer.write_all(b"\r").await?;
                }
            }
        }
    }

    writer.flush().await?;
    writer.shutdown().await?;
    reader.shutdown().await?;

    // Rename the temporary file to the original file
    fs_err::tokio::rename(format!("{filename}.tmp"), filename).await?;

    Ok((1, format!("Fixing {filename}\n").into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use bstr::ByteSlice;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    async fn create_test_file(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
        let file_path = dir.path().join(name);
        fs_err::tokio::write(&file_path, content).await.unwrap();
        file_path
    }

    async fn run_fix_on_file(file_path: &Path) -> (i32, Vec<u8>) {
        let filename = file_path.to_string_lossy().to_string();
        fix_file(&filename).await.unwrap()
    }

    #[tokio::test]
    async fn test_preserve_windows_line_endings() {
        let dir = tempdir().unwrap();

        let content = b"line1\r\nline2\r\nline3";
        let file_path = create_test_file(&dir, "windows_no_eof.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\r\nline2\r\nline3\r\n");
    }

    #[tokio::test]
    async fn test_preserve_unix_line_endings() {
        let dir = tempdir().unwrap();

        let content = b"line1\nline2\nline3";
        let file_path = create_test_file(&dir, "unix_no_eof.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\nline2\nline3\n");
    }

    #[tokio::test]
    async fn test_preserve_old_mac_line_endings() {
        let dir = tempdir().unwrap();

        let content = b"line1\rline2\rline3";
        let file_path = create_test_file(&dir, "mac_no_eof.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\rline2\rline3\r");
    }

    #[tokio::test]
    async fn test_already_has_correct_windows_ending() {
        let dir = tempdir().unwrap();

        let content = b"line1\r\nline2\r\nline3\r\n";
        let file_path = create_test_file(&dir, "windows_with_eof.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 0, "Should not change the file");
        assert!(output.is_empty());

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, content);
    }

    #[tokio::test]
    async fn test_already_has_correct_unix_ending() {
        let dir = tempdir().unwrap();

        let content = b"line1\nline2\nline3\n";
        let file_path = create_test_file(&dir, "unix_with_eof.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 0, "Should not change the file");
        assert!(output.is_empty());

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, content);
    }

    #[tokio::test]
    async fn test_empty_file() {
        let dir = tempdir().unwrap();

        let content = b"";
        let file_path = create_test_file(&dir, "empty.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 0, "Should not change empty file");
        assert!(output.is_empty());

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"");
    }

    #[tokio::test]
    async fn test_mixed_line_endings() {
        let dir = tempdir().unwrap();

        // Test file with mixed line endings (should prefer CRLF as it appears first)
        let content = b"line1\r\nline2\nline3\r\nline4";
        let file_path = create_test_file(&dir, "mixed.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\r\nline2\nline3\r\nline4\r\n");
    }

    #[tokio::test]
    async fn test_excess_newlines_removal() {
        let dir = tempdir().unwrap();

        let content = b"line1\nline2\n\n\n\n";
        let file_path = create_test_file(&dir, "excess_newlines.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\nline2\n");
    }

    #[tokio::test]
    async fn test_excess_crlf_removal() {
        let dir = tempdir().unwrap();

        let content = b"line1\r\nline2\r\n\r\n\r\n";
        let file_path = create_test_file(&dir, "excess_crlf.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\r\nline2\r\n");
    }

    #[tokio::test]
    async fn test_excess_cr_removal() {
        let dir = tempdir().unwrap();

        let content = b"line1\rline2\r\r\r";
        let file_path = create_test_file(&dir, "excess_cr.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"line1\rline2\r");
    }

    #[tokio::test]
    async fn test_all_newlines_make_empty() {
        let dir = tempdir().unwrap();

        let content = b"\n\n\n\n";
        let file_path = create_test_file(&dir, "only_newlines.txt", content).await;

        let (code, output) = run_fix_on_file(&file_path).await;

        assert_eq!(code, 1, "Should fix the file");
        assert!(output.as_bytes().contains_str("Fixing"));

        let new_content = fs_err::tokio::read(&file_path).await.unwrap();
        assert_eq!(new_content, b"");
    }
}
