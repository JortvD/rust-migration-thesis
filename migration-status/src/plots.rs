use chrono::{DateTime, Datelike, Utc};
use plotters::chart::SeriesLabelPosition;
use plotters::prelude::*;
use tokei::LanguageType;

use crate::code;
use crate::gather;

pub fn plot_language_division(
    result: &gather::TokeiStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_division.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Languages used over time for {}/{}", owner, repo), ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(start..end, 0.0f64..1.0f64)?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Percentage")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    // Collect all languages that ever appeared
    let mut languages: Vec<LanguageType> = result
        .steps
        .iter()
        .flat_map(|step| step.languages.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|lang| code::is_code_language(lang))
        .collect();

    languages.sort_by(|a, b| a.name().cmp(b.name()));

    let step_count = result.steps.len();

    // Accumulator for stacked percentages per step
    let mut accumulator: Vec<f64> = vec![0.0; step_count];

    for (idx, lang) in languages.iter().enumerate() {
        let color = Palette99::pick(idx);

        // For each language, draw a stacked "band" over time
        chart
            .draw_series(result.steps.iter().enumerate().map(|(step_idx, step)| {
                let total: usize = step.languages.iter().filter_map(|(l, v)| if code::is_code_language(l) { Some(v.code) } else { None }).sum();
                let count = step.languages.get(lang).map_or(0, |info| info.code);

                let percentage = if total > 0 {
                    count as f64 / total as f64
                } else {
                    0.0
                };

                let y0 = accumulator[step_idx];
                let y1 = y0 + percentage;
                accumulator[step_idx] = y1;

                let x0 = times[step_idx];
                let x1 = if step_idx + 1 < step_count {
                    times[step_idx + 1]
                } else {
                    end
                };

                Rectangle::new([(x0, y0), (x1, y1)], color.filled())
            }))?
            .label(format!("{:?}", lang))
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x, y - 5), (x + 10, y + 5)],
                    Palette99::pick(idx).filled(),
                )
            });
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    println!("Generated language division plot at {}", file_name);
    Ok(())
}

pub fn plot_toml_amount(
    result: &gather::CargoStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_toml_files.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let max_files = result.steps.iter().map(|s| s.num_cargo_toml).max().unwrap_or(0);
    let max_y = if max_files == 0 { 1 } else { max_files };

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("TOML files in repo {}/{} over time", owner, repo), ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(start..end, 0..max_y)?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Number of files")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    chart.draw_series(LineSeries::new(
        result.steps.iter().map(|step| (step.commit_date, step.num_cargo_toml)),
        &RED,
    ))?
    .label("Cargo.toml files")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &RED));

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    println!("Generated Cargo.toml amount plot at {}", file_name);
    Ok(())
}

pub fn plot_matching_symbols_histogram(
    result: &gather::MatchesStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.common_symbols.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_matches_histogram.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let lengths: Vec<usize> = result.common_symbols.iter().map(|m| m.len()).collect();

    let max_length = *lengths.iter().max().unwrap_or(&1) + 1;
    let mut bins = vec![0; max_length + 1];

    for &length in &lengths {
        bins[length] += 1;
    }

    let max_frequency = *bins.iter().max().unwrap_or(&1);

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Histogram for symbols present currently in Rust and in previous other languages for {}/{}", owner, repo),
            ("sans-serif", 20),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(1..max_length, 0..max_frequency)?;

    chart
        .configure_mesh()
        .x_desc("Length")
        .y_desc("Frequency")
        .x_labels(10)
        .y_labels(10)
        .draw()?;

    chart.draw_series(
        bins.iter().enumerate().map(|(length, &frequency)| {
            Rectangle::new(
                [(length, 0), (length + 1, frequency)],
                BLUE.filled(),
            )
        }),
    )?;

    root.present()?;
    println!("Generated matching symbols histogram at {}", file_name);
    Ok(())
}

pub fn plot_command_usage(
    result: &gather::CommandStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_commands.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (900, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    // Collect all commands that ever appeared
    let mut commands: Vec<String> = result
        .steps
        .iter()
        .flat_map(|step| step.command_counts.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    commands.sort();

    // Determine max usage to scale Y axis
    let max_count = result
        .steps
        .iter()
        .flat_map(|step| step.command_counts.values())
        .copied()
        .max()
        .unwrap_or(0);
    let max_y = if max_count == 0 { 1 } else { max_count };

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Commands in documentation over time for {}/{}", owner, repo),
            ("sans-serif", 20),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(start..end, 0..(max_y as i32 + 1))?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Usage count")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    for (idx, cmd) in commands.iter().enumerate() {
        let color = Palette99::pick(idx);
        let series: Vec<(DateTime<Utc>, i32)> = result
            .steps
            .iter()
            .map(|step| {
                let count = step.command_counts.get(cmd).copied().unwrap_or(0);
                (step.commit_date, count as i32)
            })
            .collect();

        chart
            .draw_series(LineSeries::new(series.clone(), ShapeStyle {
                color: color.to_rgba(),
                filled: true,
                stroke_width: 3,
            }))?
            .label(cmd.clone())
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(idx))
            });
        }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    println!("Generated command usage plot at {}", file_name);
    Ok(())
}

pub fn plot_text_analysis(
    result: &gather::TextStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_phrases.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (900, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let max_y = result
        .steps
        .iter()
        .map(|step| {
            step.copy_count.iter().sum::<usize>()
                + step.replacement_count.iter().sum::<usize>()
                + step.derivation_count.iter().sum::<usize>()
                + step.migration_count.iter().sum::<usize>()
                + step.compatibility_count.iter().sum::<usize>()
        })
        .max()
        .unwrap_or(0);

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Phrase occurrences over time for {}/{}", owner, repo),
            ("sans-serif", 20),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(start..end, 0..(max_y as i32 + 1))?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Occurrences")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    let color1 = Palette99::pick(0);
    let color2 = Palette99::pick(1);
    let color3 = Palette99::pick(2);
    let color4 = Palette99::pick(3);
    let color5 = Palette99::pick(4);

    let copy_extractor: fn(&gather::TextStepStatistics) -> usize =
        |s| s.copy_count.iter().sum::<usize>();
    let replacement_extractor: fn(&gather::TextStepStatistics) -> usize =
        |s| s.replacement_count.iter().sum::<usize>();
    let derivation_extractor: fn(&gather::TextStepStatistics) -> usize =
        |s| s.derivation_count.iter().sum::<usize>();
    let migration_extractor: fn(&gather::TextStepStatistics) -> usize =
        |s| s.migration_count.iter().sum::<usize>();
    let compatibility_extractor: fn(&gather::TextStepStatistics) -> usize =
        |s| s.compatibility_count.iter().sum::<usize>();

    let groups: [(usize, &str, PaletteColor<Palette99>, fn(&gather::TextStepStatistics) -> usize); 5] = [
        (0, "Copy Phrases", color1, copy_extractor),
        (1, "Replacement Phrases", color2, replacement_extractor),
        (2, "Derivation Phrases", color3, derivation_extractor),
        (3, "Migration Phrases", color4, migration_extractor),
        (4, "Compatibility Phrases", color5, compatibility_extractor),
    ];

    for (idx, label, color, extractor) in groups {
        let series: Vec<(DateTime<Utc>, i32)> = result
            .steps
            .iter()
            .map(|step| (step.commit_date, extractor(step) as i32))
            .collect();

        chart
            .draw_series(LineSeries::new(series, ShapeStyle {
                color: color.to_rgba(),
                filled: true,
                stroke_width: 3,
            }))?
            .label(label)
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(idx)));
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    println!("Generated phrase occurrences plot at {}", file_name);
    Ok(())
}


pub fn plot_matches2(
    result: &gather::Matches2Statistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_matches2.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (900, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let max_y = result
        .steps
        .iter()
        .map(|step| {
            step.common_pre
                .max(step.common_now)
                .max(step.overlap_that_moved)
                .max(step.moved_now)
        })
        .fold(0.0, f64::max);

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Matches2 statistics over time for {}/{}", owner, repo),
            ("sans-serif", 20),
        )
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(start..end, 0.0..(max_y + 0.1))?;

    chart
        .configure_mesh()
        .x_desc("Date")
        .x_labels(if end.year() - start.year() <= 2 { 12 * (end.year() - start.year() + 1) as usize } else { (end.year() - start.year() + 1) as usize })
        .x_label_formatter(&|dt: &DateTime<Utc>| {
            if end.year() - start.year() <= 2 {
            format!("{}/{}", dt.month(), dt.year())
            } else {
            format!("{}", dt.year())
            }
        })
        .y_desc("Value")
        .draw()?;

    let colors = [Palette99::pick(0), Palette99::pick(1), Palette99::pick(2), Palette99::pick(3)];
    let labels = [
        "Baseline symbols still present in Rust (%)", 
        "Current Rust symbols originating from baseline (%)", 
        "Overlap symbols that moved exclusively into Rust (%)", 
        "Baseline symbols now exclusive to Rust (% of Rust symbols)"
    ];
    let extractors: [fn(&gather::MatchesStepStatistics) -> f64; 4] = [
        |s| s.common_pre,
        |s| s.common_now,
        |s| s.overlap_that_moved,
        |s| s.moved_now,
    ];

    for (idx, ((label, color), extractor)) in labels.iter().zip(colors).zip(extractors).enumerate() {
        let series: Vec<(DateTime<Utc>, f64)> = result
            .steps
            .iter()
            .map(|step| (step.commit_date, extractor(step)))
            .collect();

        chart
            .draw_series(LineSeries::new(series, ShapeStyle {
                color: color.to_rgba(),
                filled: true,
                stroke_width: 3,
            }))?
            .label(*label)
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(idx)));
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;

    root.present()?;
    println!("Generated Matches2 plot at {}", file_name);
    Ok(())
}