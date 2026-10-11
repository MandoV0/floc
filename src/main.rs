use clap::Parser;
use ignore::{WalkBuilder, WalkState};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::Path,
    sync::mpsc,
};

mod scanner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum SortOptions {
    Code,
    Lines,
    Comments,
    Blanks,
    Files,
}

#[derive(Parser, Debug)]
#[command(name = "floc", version, about = "A fast source code statistics tool")]
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

    #[arg(short, long)]
    output: bool
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

    if args.output {
        write_csv(&result, args.sort);
    }

    Ok(())
}

fn scan(path: &Path, permissions: &Permission) -> io::Result<ScanResult> {
    let (tx, rx) = mpsc::channel::<(String, LanguageStats)>();

    WalkBuilder::new(path)
        .hidden(!permissions.include_hidden)
        .git_ignore(true)
        .git_exclude(true)
        .ignore(true)
        .parents(true)
        .follow_links(false)
        .build_parallel()
        .run(|| {
            let tx = tx.clone();
            Box::new(move |entry_result| {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(err) => {
                        eprintln!("ERROR: {err}");
                        return WalkState::Continue;
                    }
                };

                let Some(file_type) = entry.file_type() else {
                    return WalkState::Continue;
                };

                if file_type.is_dir() {
                    if entry.depth() > 0 {
                        let name = entry.file_name().to_string_lossy();
                        if permissions.folder_blacklist.contains(name.as_ref()) {
                            return WalkState::Skip;
                        }
                    }
                    return WalkState::Continue;
                }

                if !file_type.is_file() {
                    return WalkState::Continue;
                }

                let entry_path = entry.path();
                let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) else {
                    return WalkState::Continue;
                };

                if !permissions.extension_whitelist.contains(extension) {
                    return WalkState::Continue;
                }

                match scanner::classify_lines(entry_path) {
                    Ok(file_stats) => {
                        let _ = tx.send((extension.to_owned(), file_stats));
                    }
                    Err(err) => eprintln!("ERROR reading {}: {err}", entry_path.display()),
                }

                WalkState::Continue
            })
        });

    drop(tx);

    let mut result = ScanResult {
        languages: HashMap::new(),
    };

    for (extension, file_stats) in rx {
        let stats = result.languages.entry(extension).or_default();
        stats.files += 1;
        stats.lines += file_stats.lines;
        stats.code += file_stats.code;
        stats.comments += file_stats.comments;
        stats.blanks += file_stats.blanks;
    }

    Ok(result)
}

fn print_stats(result: &ScanResult, sort_by: SortOptions) {
    let languages = sorted_languages(result, sort_by);

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
        total.blanks,
    );
}

fn sorted_languages(result: &ScanResult, sort_by: SortOptions) -> Vec<(&String, &LanguageStats)> {
    let mut languages: Vec<_> = result.languages.iter().collect();

    languages.sort_by(|a, b| {
        let order = match sort_by {
            SortOptions::Lines => b.1.lines.cmp(&a.1.lines),
            SortOptions::Code => b.1.code.cmp(&a.1.code),
            SortOptions::Blanks => b.1.blanks.cmp(&a.1.blanks),
            SortOptions::Comments => b.1.comments.cmp(&a.1.comments),
            SortOptions::Files => b.1.files.cmp(&a.1.files),
        };

        order.then_with(|| a.0.cmp(b.0))
    });

    languages
}
fn write_csv(result: &ScanResult, sort_by: SortOptions) {
    let mut output = String::new();
    let languages = sorted_languages(result, sort_by);

    output.push_str("Languages,Files,Lines,Code,Comments,Blanks\n");
    for (lang, stats) in languages {
        output.push_str(&format!(
            "{},{},{},{},{},{}\n",
            lang, stats.files, stats.lines, stats.code, stats.comments, stats.blanks
        ));
    }
    fs::write("statistics.csv", output).expect("Failed to write csv");
}