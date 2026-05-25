use colored::Colorize;

use arictl_core::types::{DeleteResult, PreviewResult, ScanResult};

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{size} {}", UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

pub fn print_scan(result: &ScanResult) {
    println!();

    let mut grand_total_items = 0u64;

    for (cat_id, summary) in &result.categories {
        let cat_items: Vec<_> = result.items.iter().filter(|i| i.category == *cat_id).collect();
        if cat_items.is_empty() {
            continue;
        }

        grand_total_items += cat_items.len() as u64;

        println!(
            " {}  {}  {}  {}",
            "◆".cyan().bold(),
            summary.label.white().bold(),
            format_size(summary.size).yellow(),
            dim_count(cat_items.len()),
        );

        for item in &cat_items {
            println!(
                "   {}  {}  {}",
                "✓".green(),
                item.label.normal(),
                format_size(item.size).yellow(),
            );
        }
        println!();
    }

    println!(
        "{}  {}  {}  {}",
        "═".repeat(50).dimmed(),
        "Total".white().bold(),
        format_size(result.total_size).yellow().bold(),
        dim_count(grand_total_items as usize),
    );
    println!();
}

fn dim_count(n: usize) -> colored::ColoredString {
    format!("{} items", n).dimmed()
}

pub fn print_preview(result: &PreviewResult) {
    println!();
    println!("{}", "── Preview (dry run) ──".white().bold());

    for item in &result.items {
        if item.will_delete {
            println!("  {}  {}  {}", "✓".green(), item.id.normal(), format_size(item.size).yellow());
        } else {
            println!("  {}  {}  {}  {}", "✗".red(), item.id.normal(), format_size(item.size).yellow(), "(protected)".red());
        }
    }

    if !result.protected_skipped.is_empty() {
        println!();
        println!("  {} {}: {}", "⚠".yellow(), "Protected (skipped)".yellow(), result.protected_skipped.join(", ").yellow());
    }

    println!();
    println!("  {}  {}", "Total".white().bold(), format_size(result.total_size).yellow().bold());
    println!();
}

pub fn print_delete(result: &DeleteResult) {
    println!();

    for item in &result.completed {
        println!("  {}  {}  {}  {}", "✓".green(), "Deleted".green(), item.id.normal(), format_size(item.size).yellow());
    }
    for item in &result.failed {
        let reason = item.error.as_deref().unwrap_or("unknown");
        println!("  {}  {}  {}  {}", "✗".red(), "Failed".red(), item.id.normal(), reason.red());
    }

    let failed_count = result.failed.len();
    let completed_count = result.completed.len();

    println!();
    if completed_count > 0 {
        println!(
            "  {}  {}  {} {}",
            "✓".green(),
            "Freed".green().bold(),
            format_size(result.total_freed).yellow().bold(),
            format!("({} items)", completed_count).dimmed(),
        );
    }
    if failed_count > 0 {
        println!("  {}  {} {}", "✗".red(), "Failed".red().bold(), format!("({} items)", failed_count).red());
    }
    println!();
}
