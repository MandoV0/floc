use clap::{Parser, ValueEnum};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum SortOptions {
    Code,
    Lines,
    Comments,
    Blanks,
    Files,
}

#[derive(Parser, Debug)]
#[command(name = "rsdu", version, about = "A fast source code statistics tool")]
struct CliArgs {
    #[arg(default_value = ".", short, long)]
    path: String,

    #[arg(short, long, required = true, value_delimiter = ',')]
    extensions: Vec<String>,

    #[arg(short, long, value_delimiter = ',')]
    folder_blacklist: Vec<String>,

    #[arg(short, long)]
    include_hidden: bool,

    #[arg(long, value_enum, default_value_t = SortOptions::Code)]
    sort: SortOptions,
}

#[derive(Default, Debug)]
struct LanguageStats {
    files: usize,
    lines: usize,
    code: usize,
    comments: usize,
    blanks: usize,
}

struct ScanResult {
    languages: HashMap<String, LanguageStats>,
}

struct Permission {
    extension_whitelist: HashSet<String>,
    folder_blacklist: HashSet<String>,
    include_hidden: bool,
}

fn main() -> io::Result<()> {
    let args = CliArgs::parse();

    let permissions = Permission {
        extension_whitelist: args.extensions.into_iter().collect(),
        folder_blacklist: args.folder_blacklist.into_iter().collect(),
        include_hidden: args.include_hidden,
    };

    let result = scan(Path::new(&args.path), &permissions)?;
    print_stats(&result, args.sort);

    Ok(())
}

fn scan(path: &Path, permissions: &Permission) -> io::Result<ScanResult> {
    let mut result = ScanResult {
        languages: HashMap::new(),
    };

    scan_folder(path, permissions, &mut result.languages)?;

    Ok(result)
}

fn scan_folder(
    folder: &Path,
    permissions: &Permission,
    languages: &mut HashMap<String, LanguageStats>,
) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if !permissions.include_hidden && name.starts_with('.') {
            continue;
        }

        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if permissions.folder_blacklist.contains(&name) {
                continue;
            }

            scan_folder(&path, permissions, languages)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };

        if !permissions.extension_whitelist.contains(extension) {
            continue;
        }

        let file_stats = count_loc(&path)?;

        let stats = languages.entry(extension.to_owned()).or_default();
        stats.files += 1;
        stats.lines += file_stats.lines;
        stats.code += file_stats.code;
        stats.comments += file_stats.comments;
        stats.blanks += file_stats.blanks;
    }

    Ok(())
}

fn count_loc(path: &Path) -> io::Result<LanguageStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut stats = LanguageStats {
        files: 1,
        ..Default::default()
    };

    for line in reader.lines() {
        let line = line?;
        stats.lines += 1;

        let trimmed = line.trim();

        if trimmed.is_empty() {
            stats.blanks += 1;
        } else if trimmed.starts_with("//") {
            stats.comments += 1;
        } else {
            stats.code += 1;
        }
    }

    Ok(stats)
}

fn print_stats(result: &ScanResult, sort_by: SortOptions) {
    let mut languages: Vec<_> = result.languages.iter().collect();
    languages.sort_by(|a, b| a.0.cmp(b.0));


    match sort_by {
        SortOptions::Lines => {
            languages.sort_by(|a, b| { b.1.lines.cmp(&a.1.lines)});
        }
        SortOptions::Code => {
            languages.sort_by(|a, b| { b.1.code.cmp(&a.1.code)});
        }
        SortOptions::Blanks => {
            languages.sort_by(|a, b| { b.1.blanks.cmp(&a.1.blanks)});
        }
        SortOptions::Comments => {
            languages.sort_by(|a, b| { b.1.comments.cmp(&a.1.comments)});
        }
        SortOptions::Files => {
            languages.sort_by(|a, b| { b.1.files.cmp(&a.1.files)});
        }
    }

    println!(
        "{:<20} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "Language", "Files", "Lines", "Code", "Comments", "Blanks"
    );
    println!("{}", "-".repeat(80));

    let mut total = LanguageStats::default();

    for (extension, stats) in languages {
        println!(
            "{:<20} {:>8} {:>12} {:>12} {:>12} {:>12}",
            extension,
            stats.files,
            stats.lines,
            stats.code,
            stats.comments,
            stats.blanks
        );

        total.files += stats.files;
        total.lines += stats.lines;
        total.code += stats.code;
        total.comments += stats.comments;
        total.blanks += stats.blanks;
    }

    println!("{}", "-".repeat(80));
    println!(
        "{:<20} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "Total",
        total.files,
        total.lines,
        total.code,
        total.comments,
        total.blanks
    );
}