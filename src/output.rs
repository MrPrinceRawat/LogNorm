use crate::config::LogEntry;
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

pub enum Writer {
    Stdout(Box<dyn Write>),
    JsonFile(BufWriter<File>, bool), // bool tracks if we've written the opening bracket
    JsonlFile(BufWriter<File>),
    CsvFile(BufWriter<File>, bool), // bool tracks if we've written headers
    TsvFile(BufWriter<File>, bool),
}

impl Writer {
    pub fn write_batch(&mut self, logs: &[LogEntry]) -> Result<()> {
        match self {
            Writer::Stdout(writer) => {
                for log in logs {
                    writeln!(writer, "{:#?}", log)?;
                }
            }
            Writer::JsonFile(writer, is_first) => {
                if *is_first {
                    write!(writer, "[")?;
                    *is_first = false;
                } else {
                    write!(writer, ",")?;
                }

                for (i, log) in logs.iter().enumerate() {
                    if i > 0 {
                        write!(writer, ",")?;
                    }
                    let serialized = serde_json::to_string_pretty(log)?;
                    write!(writer, "\n{}", serialized)?;
                }
            }
            Writer::JsonlFile(writer) => {
                for log in logs {
                    let serialized = serde_json::to_string(log)?;
                    writeln!(writer, "{}", serialized)?;
                }
            }
            Writer::CsvFile(writer, headers_written) => {
                if !*headers_written {
                    writeln!(writer, "timestamp,host,service,level,message")?;
                    *headers_written = true;
                }

                for log in logs {
                    writeln!(
                        writer,
                        "{},{},{},{},{}",
                        escape_csv_field(&log.timestamp.as_deref().unwrap_or("")),
                        escape_csv_field(&log.host.as_deref().unwrap_or("")),
                        escape_csv_field(&log.service.as_deref().unwrap_or("")),
                        escape_csv_field(&log.level.as_deref().unwrap_or("")),
                        escape_csv_field(&log.message.as_deref().unwrap_or(""))
                    )?;
                }
            }
            Writer::TsvFile(writer, headers_written) => {
                if !*headers_written {
                    writeln!(writer, "timestamp\thost\tservice\tlevel\tmessage")?;
                    *headers_written = true;
                }

                for log in logs {
                    writeln!(
                        writer,
                        "{}\t{}\t{}\t{}\t{}",
                        escape_tsv_field(&log.timestamp.as_deref().unwrap_or("")),
                        escape_tsv_field(&log.host.as_deref().unwrap_or("")),
                        escape_tsv_field(&log.service.as_deref().unwrap_or("")),
                        escape_tsv_field(&log.level.as_deref().unwrap_or("")),
                        escape_tsv_field(&log.message.as_deref().unwrap_or(""))
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        match self {
            Writer::JsonFile(ref mut writer, _) => {
                writeln!(writer, "\n]")?;
                writer.flush()?;
            }
            Writer::JsonlFile(ref mut writer)
            | Writer::CsvFile(ref mut writer, _)
            | Writer::TsvFile(ref mut writer, _) => {
                writer.flush()?;
            }
            Writer::Stdout(ref mut writer) => {
                writer.flush()?;
            }
        }
        Ok(())
    }
}

pub fn create_writer(output_arg: &str) -> Result<Writer> {
    match output_arg {
        "stdout" => Ok(Writer::Stdout(Box::new(io::stdout()))),
        "json" => Ok(Writer::Stdout(Box::new(io::stdout()))), // JSON to stdout
        path if path.ends_with(".json") => {
            create_parent_dirs(path)?;
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            Ok(Writer::JsonFile(writer, true))
        }
        path if path.ends_with(".jsonl") || path.ends_with(".ndjson") => {
            create_parent_dirs(path)?;
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            Ok(Writer::JsonlFile(writer))
        }
        path if path.ends_with(".csv") => {
            create_parent_dirs(path)?;
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            Ok(Writer::CsvFile(writer, false))
        }
        path if path.ends_with(".tsv") => {
            create_parent_dirs(path)?;
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            Ok(Writer::TsvFile(writer, false))
        }
        path => {
            // Default to JSON file if it looks like a path
            if path.contains('/') || path.contains('\\') || path.contains('.') {
                create_parent_dirs(path)?;
                let file = File::create(path)?;
                let writer = BufWriter::new(file);
                Ok(Writer::JsonFile(writer, true))
            } else {
                Err(anyhow!(
                    "Unknown output format: {}. Use 'stdout', 'json', or a file path",
                    output_arg
                ))
            }
        }
    }
}

fn create_parent_dirs(file_path: &str) -> Result<()> {
    if let Some(parent) = Path::new(file_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn escape_tsv_field(field: &str) -> String {
    field
        .replace('\t', " ")
        .replace('\n', " ")
        .replace('\r', " ")
}

// Legacy function for backward compatibility
pub fn write(output_arg: &str, logs: &[LogEntry]) -> Result<()> {
    let mut writer = create_writer(output_arg)?;
    writer.write_batch(logs)?;
    writer.finish()
}
