use std::{error, fs};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use glob::{glob_with, MatchOptions};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use zip::write::FileOptions;

const STYLE: &str = "[{elapsed_precise} {wide_bar:.green/blue}] {pos:5}/{len:5}";
const PROGRESS_CHARS: &str = "##-";

/// This application conditionally extracts files in a target folder and stores a certain number of files in a ZIP file.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// filename pattern matching (glob)
    pattern: String,
    /// Destination Folder
    dst: String,
    /// Number of saves per file
    #[arg(short, long, default_value_t = 1000)]
    file_count_per_file: i32,
    /// Is it case-sensitive
    #[arg(long, action = clap::ArgAction::SetFalse)]
    case_sensitive: bool,
    /// Match with a literal /
    #[arg(long)]
    require_literal_separator: bool,
    /// Whether or not paths that contain components that start with a . will require that . appears literally in the pattern
    #[arg(long)]
    require_literal_leading_dot: bool,
}

fn get_file_as_byte_vec(filename: PathBuf) -> Result<Vec<u8>, std::io::Error> {
    let mut f = File::open(&filename)?;
    let metadata = fs::metadata(&filename)?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer)?;
    Ok(buffer)
}

fn search_files(args: Args) -> Result<Vec<PathBuf>, Box<dyn error::Error>> {
    let mut options = MatchOptions::new();
    options.case_sensitive = args.case_sensitive;
    options.require_literal_leading_dot = args.require_literal_leading_dot;
    options.require_literal_separator = args.require_literal_separator;
    let files = glob_with(args.pattern.as_str(), options)?
        .map(|e| e.unwrap())
        .collect::<Vec<_>>();
    Ok(files)
}

fn divide_files(files: Vec<PathBuf>, file_count: usize) -> Vec<Vec<PathBuf>> {
    files
        .chunks(file_count)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let args = Args::parse();
    let dst = Path::new(args.dst.as_str());
    if dst.is_dir() {
        if !dst.read_dir()?.next().is_none() {
            println!("Destination folder is not empty.");
            return Ok(());
        }
    } else {
        fs::create_dir_all(dst.join("zip"))?;
    }
    let files = search_files(args.clone())?;
    let divided_files = divide_files(files.clone(), args.file_count_per_file as usize);
    let bars = MultiProgress::new();
    let block_pb = bars.add(ProgressBar::new(divided_files.len() as u64));
    block_pb.set_style(
        ProgressStyle::default_bar()
            .template(&*("Blocks: ".to_owned() + STYLE))?
            .progress_chars(PROGRESS_CHARS),
    );
    let file_pb = bars.add(ProgressBar::new(files.len() as u64));
    file_pb.set_style(
        ProgressStyle::default_bar()
            .template(&*("Files : ".to_owned() + STYLE))?
            .progress_chars(PROGRESS_CHARS),
    );
    let writer = BufWriter::new(File::create(dst.join("results.csv"))?);
    let mut writer = csv::Writer::from_writer(writer);
    writer.write_record(&["zip", "filename"])?;
    for i in 0..divided_files.len() {
        let block: &Vec<PathBuf> = divided_files.get(i).unwrap();
        let filename = format!("{}_{}.zip", dst.file_name().unwrap().to_str().unwrap(), i);
        let path = dst.join("zip").join(filename.clone());
        let file = File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        for item in block {
            let item_name = String::from(item.file_name().unwrap().to_str().unwrap());
            zip.start_file(item_name.clone(), options)?;
            zip.write_all(&*get_file_as_byte_vec(item.clone().to_path_buf())?)?;
            writer.write_record(&[format!("{}.zip", i), item_name])?;
            file_pb.inc(1);
        }
        zip.finish()?;
        block_pb.inc(1);
    }
    Ok(())
}
